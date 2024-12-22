/// Este módulo define la estructura `Stock` para gestionar el inventario de sabores de helado.
///
/// La estructura `Stock` incluye una tabla de stock por sabor de helado representada como un `HashMap`,
/// junto con una marca de tiempo que registra la última modificación realizada en el inventario.
///
/// Proporciona métodos para inicializar el stock con cantidades predeterminadas, verificar si hay
/// suficiente stock para satisfacer requisitos específicos, y añadir o restar cantidades de stock,
/// actualizando la marca de tiempo de la última modificación en cada operación.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

use crate::common::constants::INITIAL_GRAMS_AMOUNT;
use crate::common::flavors::IceCreamFlavor;

/// Estructura que representa el inventario de stock de diferentes sabores de helado.

#[derive(Serialize, Deserialize, Debug)]
pub struct Stock {
    pub stock_table: HashMap<IceCreamFlavor, i32>,
    pub last_modification_timestamp: SystemTime,
}

impl Stock {
    /// Método para crear un nuevo inventario de stock inicializado con cantidades iniciales iguales para todos los sabores.

    pub fn new() -> Self {
        let mut stock_table = HashMap::new();
        stock_table.insert(IceCreamFlavor::Vanilla, INITIAL_GRAMS_AMOUNT);
        stock_table.insert(IceCreamFlavor::Chocolate, INITIAL_GRAMS_AMOUNT);
        stock_table.insert(IceCreamFlavor::Strawberry, INITIAL_GRAMS_AMOUNT);
        stock_table.insert(IceCreamFlavor::Mint, INITIAL_GRAMS_AMOUNT);
        Stock {
            stock_table,
            last_modification_timestamp: SystemTime::now(),
        }
    }

    /// Verifica si hay suficiente stock disponible para satisfacer los requisitos especificados.

    pub fn has_enough_stock(&self, required: &HashMap<IceCreamFlavor, i32>) -> bool {
        for (flavor, &amount) in required.iter() {
            if let Some(&current_stock) = self.stock_table.get(flavor) {
                if current_stock < amount {
                    return false;
                }
            } else {
                return false;
            }
        }
        return true;
    }

    /// Resta cantidades especificadas del stock actual y actualiza la marca de tiempo de la última modificación.

    pub fn subtract_stock(&mut self, to_subtract: &HashMap<IceCreamFlavor, i32>) -> SystemTime {
        for (flavor, &amount) in to_subtract.iter() {
            if let Some(stock) = self.stock_table.get_mut(flavor) {
                *stock -= amount;
            }
        }
        let now = SystemTime::now();
        self.last_modification_timestamp = now;
        now
    }

    /// Añade cantidades especificadas al stock actual y actualiza la marca de tiempo de la última modificación.

    pub fn add_stock(&mut self, to_add: &HashMap<IceCreamFlavor, i32>) -> SystemTime {
        for (flavor, &amount) in to_add.iter() {
            if let Some(stock) = self.stock_table.get_mut(flavor) {
                *stock += amount;
            }
        }
        let now = SystemTime::now();
        self.last_modification_timestamp = now;
        now
    }

    /// Resta cantidades especificadas del stock actual utilizando la marca de tiempo especificada.

    pub fn subtract_with_timestamp(
        &mut self,
        to_subtract: &HashMap<IceCreamFlavor, i32>,
        timestamp: SystemTime,
    ) {
        for (flavor, &amount) in to_subtract.iter() {
            if let Some(stock) = self.stock_table.get_mut(flavor) {
                *stock -= amount;
            }
        }
        self.last_modification_timestamp = timestamp;
    }

    /// Añade cantidades especificadas al stock actual utilizando la marca de tiempo especificada.

    pub fn add_with_timestamp(
        &mut self,
        to_add: &HashMap<IceCreamFlavor, i32>,
        timestamp: SystemTime,
    ) {
        for (flavor, &amount) in to_add.iter() {
            if let Some(stock) = self.stock_table.get_mut(flavor) {
                *stock += amount;
            }
        }
        self.last_modification_timestamp = timestamp;
    }
}
