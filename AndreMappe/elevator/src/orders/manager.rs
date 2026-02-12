use std::collections::VecDeque;
use crossbeam_channel as cbc;

use crate::messages::{ Command, FsmEvent};
use driver_rust::elevio::{elev as e, poll};

pub struct OrderManager {
    orders: VecDeque<poll::CallButton>,
    fsm_cmd_tx: cbc::Sender<Command>,
    fsm_idle: bool,
    elevator: e::Elevator,
}

impl OrderManager {
    pub fn new(elevator: e::Elevator, fsm_cmd_tx: cbc::Sender<Command>) -> Self {
        Self {
            orders: VecDeque::new(),
            fsm_cmd_tx,
            fsm_idle: true,
            elevator,
        }
    }

    pub fn new_call(&mut self, order: poll::CallButton) {
        println!("New order: {:?}", order);
        self.elevator.call_button_light(order.floor, order.call, true);
        self.orders.push_back(order);
        self.try_dispatch();
    }

    pub fn handle_fsm_event(&mut self, event: FsmEvent) {
        match event {
            FsmEvent::Idle => {
                self.fsm_idle = true;
                self.try_dispatch();
            }
            _ => {}
        }
    }


    fn try_dispatch(&mut self) {
        println!("Tried to dispatch");
        if self.fsm_idle {
            println!("Elevator was able to dispatch");
            if let Some(order) = self.orders.pop_front() {

                let cmd = Command::GoToFloor(order.floor);
                self.fsm_cmd_tx.send(cmd).ok();
                self.fsm_idle = false;
            } else {
                println!("No more orders")
            }
        } else {
                println!("Elevator was not able to dispatch")
        }
    }
}
