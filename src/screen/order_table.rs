//use heladeria::common::flavors::IceCreamFlavor;
use serde::{Deserialize, Serialize};
//use serde_json;
use std::collections::HashMap;

use crate::common::flavors::IceCreamFlavor;

/// Representa una tabla de pedidos, mapeando order_id a (screen_id, detalles, completado).

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderTable {
    orders: HashMap<i32, (i32, HashMap<IceCreamFlavor, i32>, bool)>, // order_id -> (screen_id, details, boolean)
}

impl OrderTable {
    /// Crea una nueva OrderTable vacía.
    pub fn new() -> Self {
        OrderTable {
            orders: HashMap::new(),
        }
    }

    // Create a table from a JSON string
    // pub fn from_json(json_str: &str) -> Self {
    //     serde_json::from_str(json_str).unwrap()
    // }

    /// Agrega un pedido a la tabla.
    pub fn add_order(
        &mut self,
        screen_id: i32,
        order_id: i32,
        details: HashMap<IceCreamFlavor, i32>,
    ) {
        self.orders.insert(order_id, (screen_id, details, false)); // Initialize boolean as false
    }

    // Mark an order as completed
    // pub fn mark_order_completed(&mut self, order_id: i32) {
    //     if let Some(order) = self.orders.get_mut(&order_id) {
    //         order.2 = true;
    //     }
    // }

    /// Elimina un pedido de la tabla.
    pub fn remove_order(&mut self, order_id: i32) -> Option<(i32, HashMap<IceCreamFlavor, i32>)> {
        if let Some((screen_id, details, _)) = self.orders.remove(&order_id) {
            Some((screen_id, details))
        } else {
            None
        }
    }

    /// Transfiere pedidos de una pantalla a otra.
    pub fn transfer_orders(&mut self, from_screen_id: i32, to_screen_id: i32) -> Vec<i32> {
        let mut changed_orders = Vec::new();
        for (order_id, (screen_id, _, _)) in self.orders.iter_mut() {
            if *screen_id == from_screen_id {
                *screen_id = to_screen_id;
                changed_orders.push(*order_id);
            }
        }
        changed_orders
    }

    // // Serialize the structure to JSON for sending over TCP
    // pub fn serialize(&self) -> String {
    //     serde_json::to_string(&self).unwrap()
    // }
}
