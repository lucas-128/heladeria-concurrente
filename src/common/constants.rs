/// Dirección del Gateway.
pub const GATEWAY_ADDRESS: &str = "127.0.0.1:6000";

/// Tamaño del búfer.
pub const BUFFER_SIZE: usize = 4096;

/// Cantidad inicial de gramos disponibles para cada sabor de helado.
pub const INITIAL_GRAMS_AMOUNT: i32 = 10000;

/// Factor de tiempo de preparación de helado.
pub const SLEEP_FACTOR: u64 = 10;

/// Cantidad inical de pedidos que lanza la pantalla
pub const FIRST_BATCH: usize = 1;

/// Cantidad de pedidos que lanza la pantalla por cada pedido terminado
pub const MULTIPLICATION_BATCH: usize = 2;
