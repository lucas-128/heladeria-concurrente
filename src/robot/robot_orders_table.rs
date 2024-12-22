/// Este módulo define estructuras y métodos para gestionar y manipular órdenes de helados en un sistema distribuido de robots.
///
/// Contiene las estructuras `Order`, `OrdersList` y `OrdersTable`, serializables/deserializables mediante serde, que representan órdenes individuales, listas de órdenes y tablas de órdenes respectivamente.
///
/// La estructura `OrdersList` proporciona métodos para añadir, eliminar y gestionar órdenes dentro de una lista para un robot específico.
///
/// La estructura `OrdersTable` gestiona múltiples listas de órdenes asociadas a diferentes robots, permitiendo inicialización, adición, eliminación y consultas sobre las órdenes.
///
/// Incluye métodos para determinar el robot con menos órdenes y para eliminar robots y sus órdenes de la tabla.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::common::flavors::IceCreamFlavor;

/// Estructura que representa una orden de helado, con un identificador único y detalles de la orden por sabor.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub order_id: usize,
    pub order_details: HashMap<IceCreamFlavor, i32>,
}

/// Estructura que representa una lista de órdenes de helado para un robot específico, almacenadas en un vector.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrdersList {
    pub orders: Vec<Order>,
}

impl OrdersList {
    /// Método para crear una nueva lista de órdenes vacía.
    pub fn new() -> Self {
        OrdersList { orders: Vec::new() }
    }

    /// Método para añadir una nueva orden a la lista.
    pub fn add_order(&mut self, order_id: usize, order_details: HashMap<IceCreamFlavor, i32>) {
        self.orders.push(Order {
            order_id,
            order_details,
        });
    }

    /// Método para eliminar una orden por su identificador.
    pub fn remove_order(&mut self, order_id: usize) {
        self.orders.retain(|order| order.order_id != order_id);
    }
}
/// Estructura que representa una tabla de órdenes de helado para múltiples robots, mapeando IDs de robots a listas de órdenes.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrdersTable {
    pub orders_map: HashMap<usize, OrdersList>, // robot id, orders list
}

impl OrdersTable {
    /// Método para crear una nueva tabla de órdenes vacía.
    pub fn new() -> Self {
        OrdersTable {
            orders_map: HashMap::new(),
        }
    }

    /// Método para inicializar la tabla con listas de órdenes vacías para `n` robots.
    pub fn initialize(&mut self, n: usize) {
        for robot_id in 0..n {
            self.orders_map.insert(robot_id, OrdersList::new());
        }
    }

    /// Método para obtener la lista de órdenes de un robot específico por su ID.
    pub fn get_robot_orders(&self, robot_id: usize) -> Option<OrdersList> {
        self.orders_map.get(&robot_id).cloned()
    }

    /// Método para añadir una orden a la lista de órdenes de un robot específico.
    pub fn add_order_for_robot(
        &mut self,
        robot_id: usize,
        order_id: usize,
        order_details: HashMap<IceCreamFlavor, i32>,
    ) {
        if let Some(orders_list) = self.orders_map.get_mut(&robot_id) {
            orders_list.add_order(order_id, order_details);
        } else {
            let mut new_orders_list = OrdersList::new();
            new_orders_list.add_order(order_id, order_details);
            self.orders_map.insert(robot_id, new_orders_list);
        }
    }

    /// Método para eliminar una orden de la lista de órdenes de un robot específico por su ID.
    pub fn remove_order_for_robot(&mut self, robot_id: usize, order_id: usize) {
        if let Some(orders_list) = self.orders_map.get_mut(&robot_id) {
            orders_list.remove_order(order_id);
        }
    }

    /// Método para obtener el ID del robot con menos órdenes (tamaño de OrdersList más pequeño).
    pub fn robot_with_least_orders(&self) -> Option<usize> {
        let mut min_orders_count = usize::MAX;
        let mut min_robot_id = None;

        for (&robot_id, orders_list) in &self.orders_map {
            let orders_count = orders_list.orders.len();
            if orders_count < min_orders_count {
                min_orders_count = orders_count;
                min_robot_id = Some(robot_id);
            }
        }

        min_robot_id
    }

    /// Método para eliminar un robot y todas sus órdenes de la tabla.
    pub fn remove_robot(&mut self, robot_id: usize) {
        self.orders_map.remove(&robot_id);
    }
}
