/// Este módulo define diferentes tipos de mensajes utilizados en la aplicación para la comunicación entre robots, pantallas y el sistema central.
///
/// Contiene estructuras y enumeraciones serializables/deserializables que representan diversos eventos y datos intercambiados entre las entidades del sistema distribuido.
///
/// Las funciones `serialize_message` y `deserialize_message` permiten convertir estos tipos de mensajes en bytes y viceversa, facilitando la comunicación a través de la red.
use super::flavors::IceCreamFlavor;
use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::SystemTime};

/// Enumeración que define diferentes tipos de mensajes utilizados en la aplicación.
#[derive(Serialize, Deserialize, Debug)]
pub enum MessageType {
    DeadRobot(DeadRobot),
    DeadScreen(DeadScreen),
    NewLeader(NewLeader),
    Election(Election),
    RemoveRobot(usize),
    AllConnected(AllConnected),
    Token(Token),
    OrderScreen(OrderScreen),
    Order(Order),
    Prepare(Prepare),
    Kill(),
    NewOrder(OrderData),
    RobotIntroduction(RobotIntroduction),
    ScreenIntroduction(ScreenIntroduction),
    OrderComplete(OrderComplete),
    OrderDelivered(OrderDelivered),
    UpdateStock(UpdateData),
    PossibleLostToken(TokenData),
    TokenFound(IceCreamFlavor),
    Commit(Commit),
    Abort(Abort),
    NewLeaderIntroduction(usize),
    UpdateRobotLeader(usize),
    UpdateScreenLeader(usize),
}
/// Estructura que representa un robot que ha dejado de funcionar.

#[derive(Serialize, Deserialize, Debug)]
pub struct DeadRobot {
    pub sender_id: usize,
    pub dead_robot_id: usize,
}
/// Estructura que representa una pantalla que ha dejado de funcionar.

#[derive(Serialize, Deserialize, Debug)]
pub struct DeadScreen {
    pub sender_id: usize,
    pub dead_screen_id: usize,
}
/// Estructura que representa un pedido realizado a una pantalla.

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderScreen {
    pub sender_id: usize,
    pub order_id: usize,
    pub order_details: HashMap<IceCreamFlavor, i32>,
}
/// Estructura que representa un pedido genérico.

#[derive(Serialize, Deserialize, Debug)]
pub struct Order {
    pub order_id: usize,
    pub order_details: HashMap<IceCreamFlavor, i32>,
}

/// Estructura que representa una orden de aborto de un pedido.

#[derive(Serialize, Deserialize, Debug)]
pub struct Abort {
    pub order_id: usize,
}
/// Estructura que representa una confirmación de un pedido completado.

#[derive(Serialize, Deserialize, Debug)]
pub struct Commit {
    pub order_id: usize,
}
/// Estructura que contiene datos actualizados de stock.

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateData {
    pub modified_values: HashMap<IceCreamFlavor, i32>,
    pub timestamp: SystemTime,
    pub subtract: bool,
}

/// Estructura que representa la finalización de un pedido por un robot.

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderComplete {
    pub robot_id_maker: usize,
    pub order_id: usize,
}

/// Estructura que representa la entrega de un pedido por un robot.

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderDelivered {
    pub robot_id_maker: usize,
    pub order_id: usize,
}

/// Estructura que contiene datos de un nuevo pedido.

#[derive(Serialize, Deserialize, Debug)]
pub struct OrderData {
    pub target_id: usize,
    pub order_id: usize,
    pub order_details: HashMap<IceCreamFlavor, i32>,
}

/// Estructura que representa un mensaje de nuevo líder.

#[derive(Serialize, Deserialize, Debug)]
pub struct NewLeader {
    pub sender_id: usize,
    pub new_leader_id: usize,
    pub dead_leader_id: usize,
}

/// Estructura que representa un mensaje de elección de líder.

#[derive(Serialize, Deserialize, Debug)]
pub struct Election {
    pub sender_id: usize,
    pub current_candidate_id: usize,
    pub dead_leader_id: usize,
}

/// Estructura que representa un mensaje de todos conectados.

#[derive(Serialize, Deserialize, Debug)]
pub struct AllConnected {
    pub sender_id: usize,
}

/// Estructura que representa un token intercambiado entre entidades.

#[derive(Serialize, Deserialize, Debug)]
pub struct Token {
    pub sender_id: usize,
    pub flavour: IceCreamFlavor,
    pub last_modified_by_id: usize,
    pub last_modification_timestamp: SystemTime,
    pub available_ammount: i32, // grams
}

/// Estructura que representa una introducción de robot.

#[derive(Serialize, Deserialize, Debug)]
pub struct RobotIntroduction {
    pub sender_id: usize,
}

/// Estructura que representa una introducción de pantalla.

#[derive(Serialize, Deserialize, Debug)]
pub struct ScreenIntroduction {
    pub sender_id: usize,
}

/// Estructura que representa un mensaje de preparación de pedido.

#[derive(Serialize, Deserialize, Debug)]
pub struct Prepare {
    pub sender_id: usize,
    pub target_id: usize, // robot id that makes the order
    pub order_id: usize,
    pub order_details: HashMap<IceCreamFlavor, i32>,
}

/// Estructura que contiene datos de un token.

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenData {
    pub flavor: IceCreamFlavor,
    pub timestamp: SystemTime,
    pub stock: i32,
}

/// Estructura que representa un mensaje de tipo `Kill`, utilizado para terminar la ejecución de un hilo.

#[derive(Serialize, Deserialize, Debug)]
pub struct Kill {}

/// Serializa un mensaje `MessageType` a bytes usando bincode.
pub fn serialize_message(message: &MessageType) -> Option<Vec<u8>> {
    match serialize(message) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("Serialization error: {:?}", err);
            None
        }
    }
}

/// Deserializa bytes en un mensaje `MessageType` usando bincode.
pub fn deserialize_message(bytes: &[u8]) -> Option<MessageType> {
    match deserialize(bytes) {
        Ok(message) => Some(message),
        Err(err) => {
            eprintln!("Deserialization error: {:?}", err);
            None
        }
    }
}
