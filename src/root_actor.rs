use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use actix::prelude::*;
use log::info;
use rand::seq::{IndexedRandom, SliceRandom};
use tokio::sync::oneshot;

use crate::location_actor::LocationActor;

const INITIAL_POOL_SIZE: usize = 15000;
const POOL_REFRESH_THRESHOLD: f32 = 0.3; 
const REFRESH_BATCH_SIZE: usize = (POOL_REFRESH_THRESHOLD * INITIAL_POOL_SIZE as f32) as usize; 
const ARBITER_POOL_SIZE: usize = 2;

pub struct RootActor {
    addrs: HashMap<String, Addr<LocationActor>>,
    pool: VecDeque<Addr<LocationActor>>,
    refresh_actor: Addr<PoolRefreshActor>,
    is_refreshing: bool,
}


#[derive(Message)]
#[rtype(result = "()")]
struct RefreshPool {
    caller: Addr<RootActor>,
    batch_size: usize,
}


#[derive(Message)]
#[rtype(result = "()")]
struct PoolRefreshComplete {
    new_actors: Vec<Addr<LocationActor>>,
    is_initial_warmup: bool,
}


pub struct PoolRefreshActor {
    arbiter_pool: Vec<Arbiter>,
}

impl Actor for PoolRefreshActor {
    type Context = SyncContext<Self>;
}

impl Handler<RefreshPool> for PoolRefreshActor {
    type Result = ();

    fn handle(&mut self, msg: RefreshPool, _: &mut SyncContext<Self>) -> Self::Result {
        let batch_size = msg.batch_size;
        let is_initial_warmup = batch_size == INITIAL_POOL_SIZE;
        
        info!(
            "{} creation of {} location actors", 
            if is_initial_warmup { "Starting initial" } else { "Starting refresh" },
            batch_size
        );
        
        let mut new_actors = Vec::with_capacity(batch_size);
        
        for _ in 0..batch_size {
            let mut rng = rand::thread_rng();
            let arbiter = self.arbiter_pool.choose(&mut rng).unwrap();
            let addr = LocationActor::start_in_arbiter(&arbiter.handle(), |_| LocationActor::new());
            new_actors.push(addr);
        }
        
        info!(
            "{} completed, sending {} new actors back to root actor",
            if is_initial_warmup { "Initial creation" } else { "Refresh" },
            new_actors.len()
        );
        
        
        msg.caller.do_send(PoolRefreshComplete {
            new_actors,
            is_initial_warmup,
        });
    }
}

impl PoolRefreshActor {
    fn new() -> Self {
        let mut arbiter_pool = Vec::with_capacity(ARBITER_POOL_SIZE);
        for _ in 0..ARBITER_POOL_SIZE {
            let arbiter = Arbiter::new();
            arbiter_pool.push(arbiter);
        }
        
        PoolRefreshActor {
            arbiter_pool,
        }
    }
}

impl RootActor {
    pub fn new() -> Self {
        
        let refresh_actor = SyncArbiter::start(1, || PoolRefreshActor::new());

        let root_actor = RootActor {
            addrs: HashMap::new(),
            pool: VecDeque::with_capacity(INITIAL_POOL_SIZE),
            refresh_actor,
            is_refreshing: true, 
        };

        
        root_actor
    }
    
    fn check_pool_size(&mut self, ctx: &mut SyncContext<Self>) {
        
        let current_size = self.pool.len();
        let capacity_used = 1.0 - (current_size as f32 / INITIAL_POOL_SIZE as f32);
        
        if capacity_used >= POOL_REFRESH_THRESHOLD && !self.is_refreshing {
            info!("Pool usage at {}%, triggering refresh", capacity_used * 100.0);
            self.is_refreshing = true;
            
            
            self.refresh_actor.do_send(RefreshPool {
                caller: ctx.address(),
                batch_size: REFRESH_BATCH_SIZE,
            });
        }
    }
}

impl Actor for RootActor {
    type Context = SyncContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("RootActor started, initiating pool warmup");
        
        
        self.refresh_actor.do_send(RefreshPool {
            caller: ctx.address(),
            batch_size: INITIAL_POOL_SIZE,
        });
    }
}


impl Handler<PoolRefreshComplete> for RootActor {
    type Result = ();

    fn handle(&mut self, msg: PoolRefreshComplete, _: &mut SyncContext<Self>) -> Self::Result {
        if msg.is_initial_warmup {
            info!("Initial pool warmup completed with {} actors", msg.new_actors.len());
        } else {
            info!("Pool refresh completed with {} new actors", msg.new_actors.len());
        }
        
        
        for addr in msg.new_actors {
            self.pool.push_back(addr);
        }
        
        
        self.is_refreshing = false;
        info!("Pool current size: {}", self.pool.len());
    }
}

#[derive(Message)]
#[rtype(result = "Result<Addr<LocationActor>, ()>")]
pub struct GetAddr(pub String);

impl Handler<GetAddr> for RootActor {
    type Result = Result<Addr<LocationActor>, ()>;

    fn handle(&mut self, msg: GetAddr, ctx: &mut SyncContext<Self>) -> Self::Result {
        if let Some(addr) = self.addrs.get(&msg.0) {
            info!("Actor found for location_id: '{}'", msg.0);
            return Ok(addr.clone());
        }
        
        info!("Creating actor for location_id: '{}'", msg.0);
        
        let addr = if let Some(addr) = self.pool.pop_front() {
            addr
        } else {
            info!("Pool is empty, waiting for refresh to complete");
            LocationActor::new().start()
        };

        self.addrs.insert(msg.0.clone(), addr.clone());
        
        
        self.check_pool_size(ctx);
        
        Ok(addr)
    }
}
