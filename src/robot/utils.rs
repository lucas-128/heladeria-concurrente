use super::robot::Robot;
use crate::common::{constants::BUFFER_SIZE, flavors::IceCreamFlavor, messages::*};
use std::{io::Read, net::TcpStream, sync::Arc, thread, time::SystemTime};

/// Maneja una conexión TCP entrante, esperando un mensaje de introducción que determina
/// si el nodo conectado es un robot o una pantalla. Según el tipo de nodo,
/// llama a funciones específicas para manejar mensajes entrantes adicionales.
pub fn handle_incoming_connection(robot: Arc<Robot>, mut socket: TcpStream, my_id: usize) {
    let robot_ref = robot.clone();
    let _tx_sender = robot.tx_sender_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer = Vec::new();

    let robot_ref_c = robot_ref.clone();
    match socket.read(&mut buffer) {
        Ok(0) => {
            //Connection closed
        }
        Ok(n) => {
            read_buffer.extend_from_slice(&buffer[..n]);
            match deserialize_message(&buffer) {
                Some(message) => {
                    match message {
                        MessageType::RobotIntroduction(_) => {
                            //println!("Robot introduction received!");
                            handle_robot_connection(
                                robot_ref_c,
                                my_id,
                                socket.try_clone().unwrap(),
                            );
                        }

                        MessageType::ScreenIntroduction(screen_data) => {
                            //println!("Recibido una Screen Introduction");
                            if robot.is_leader(my_id) {
                                //println!("soy lider, manejo la conexion.");
                                handle_screen_connection(
                                    robot,
                                    my_id,
                                    socket.try_clone().unwrap(),
                                    screen_data.sender_id,
                                    false,
                                )
                            } else {
                                //println!("No soy lider. No manejo la conexion");
                            }
                        }
                        MessageType::NewLeaderIntroduction(screen_id) => {
                            //println!("reciido new leader intro...");
                            if robot.is_leader(my_id) {
                                //println!("soy lider, manejo la new conexion.");
                                handle_screen_connection(
                                    robot,
                                    my_id,
                                    socket.try_clone().unwrap(),
                                    screen_id,
                                    true,
                                )
                            } else {
                                //println!("No soy lider. No manejo la new conexion");
                            }
                        }
                        _ => {
                            //eprintln!("Conection requires introduction: {:?}", read_buffer);
                        }
                    }
                    read_buffer.clear(); // Clear the buffer
                }
                None => {
                    //println!("No value present");
                }
            }
        }
        Err(e) => {
            eprintln!("Robot {}: Failed to read from socket; err = {:?}", my_id, e);
        }
    }
}

/// Maneja los mensajes entrantes de un nodo robot a través de una conexión TCP.
///
/// Esta función lee mensajes desde el socket y los procesa según su tipo.
/// Puede enviar mensajes de preparación a otros nodos o manejar mensajes
/// especiales como la detección de nodos caídos o la reasignación de órdenes perdidas.
/// Si el mensaje es de tipo "Prepare", se prepara la orden correspondiente si el destino
/// coincide con el ID del robot actual. En caso contrario, se llama a la función
/// `handle_other_messages` para seguir procesando el mensaje recibido.
///
pub fn handle_robot_connection(robot: Arc<Robot>, my_id: usize, mut socket: TcpStream) {
    let robot_ref = robot.clone();
    let tx_sender = robot.tx_sender_channel.clone();
    let tx_prepare = robot.tx_prepare_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer = Vec::new();

    loop {
        let robot_ref_c = robot_ref.clone();
        match socket.read(&mut buffer) {
            Ok(0) => {
                // Murio el robot donde yo estaba leyendo mensajes.
                let prev_robot_id = robot_ref_c.find_prev_robot(my_id);
                // prev robot id es el que murio. Tengo que notificar su baja.
                if let Some(dead_id) = prev_robot_id {
                    println!("ID Robot Muerto: {}", dead_id);
                    if robot.is_net_size_2() {
                        // si el tamano de la red era 2 y murio el robot, estoy solo.
                        // update network
                        Robot::update_network(robot.clone(), dead_id);

                        // kill old sender thread
                        let _ = tx_sender.send(MessageType::Kill());

                        // start another sender thread
                        let robot_ref = robot.clone();
                        thread::spawn(move || {
                            robot_ref.connect_to_next_robot();
                        });

                        if robot.is_leader(my_id) {
                        } else {
                            robot.set_new_leader(my_id, my_id);
                            // screen sender thread start todo!()
                        }

                        for flavor in IceCreamFlavor::iter() {
                            // para cada gusto. Me fijo si se perdio el token.
                            if robot.has_token(flavor) {
                                //println!("Yo tengo el token de {:?}, no se perdio...", flavor);
                                // Yo tengo el token, no se perdio.
                            } else {
                                // Emitir token perdido.
                                println!("No se encontro el token de {:?}", flavor);
                                let current_stock = robot.get_flavor_stock(flavor);
                                let lost_flavour_token = MessageType::Token(Token {
                                    sender_id: my_id,
                                    flavour: flavor.clone(),
                                    last_modified_by_id: my_id,
                                    last_modification_timestamp: SystemTime::now(),
                                    available_ammount: current_stock.clone(),
                                });
                                // println!(
                                //     "Nuevo token de {:?} emitido con stock: {:?}",
                                //     flavor, current_stock
                                // );
                                let _ = tx_sender.send(lost_flavour_token);
                            }
                        }

                        // Buscar ordenes en la tabla de robot muerto.
                        let lost_orders = robot.get_robot_orders(dead_id);
                        println!(
                            "Las siguientes ordenes las estaba laburando el robot muerto: {:?},",
                            lost_orders
                        );

                        robot.remove_dead_from_orders_table(dead_id);
                        // Stock recuperado por los pedidos perdidos.
                        recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);
                        // Para los pedidos perdidos, los intento re asignar.
                        reassign_lost_orders(lost_orders, &robot, my_id);

                        // Inicio screen sender thread con pantalla
                        let arc_robot = robot.clone();
                        thread::spawn(move || arc_robot.connect_to_screen(true));
                    } else {
                        // Notificar al siguiente robot.
                        let deadrobot_msg = MessageType::DeadRobot(DeadRobot {
                            sender_id: my_id,
                            dead_robot_id: dead_id,
                        });
                        let _ = tx_sender.send(deadrobot_msg);

                        // Si el id muerto es el lider, hay que elegir nuevo lider
                        if robot_ref.is_leader(dead_id) {
                            robot_ref.send_leader_election_message(my_id, dead_id);
                        }

                        // Update network
                        Robot::update_network(robot.clone(), dead_id);
                    }

                    // Si soy el lider. Revisar si se perdieron tokens.
                    if robot.is_leader(my_id) {
                        println!("Robot muerto detectado. Ver si se perdieron tokens.");
                        recover_lost_tokens(&robot, &tx_sender);

                        // Soy lider, tengo que re asignar pedidos que pertenecian al robot muerto.
                        // buscar ordenes en la tabla de ese robot.
                        let lost_orders = robot.get_robot_orders(dead_id);
                        println!(
                            "Las siguientes ordenes correspondian al robot muerto: {:?},",
                            lost_orders
                        );

                        robot.remove_dead_from_orders_table(dead_id);
                        let remove_robot_msg = MessageType::RemoveRobot(dead_id.clone());
                        let _ = tx_sender.send(remove_robot_msg);

                        // Stock recuperado por los pedidos perdidos.
                        recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);

                        // Para los pedidos perdidos, los intento re asignar.
                        reassign_lost_orders(lost_orders, &robot, my_id);
                    }
                    break;
                } else {
                    eprintln!("No Value Found");
                }
                break;
            }
            Ok(n) => {
                read_buffer.extend_from_slice(&buffer[..n]);
                match deserialize_message(&buffer) {
                    Some(message) => {
                        match message {
                            MessageType::Prepare(ref order_data) => {
                                if order_data.target_id == my_id {
                                    let _ = tx_prepare.send(message);
                                } else {
                                    let _ = tx_sender.send(message);
                                }
                            }

                            _ => {
                                thread::spawn(move || {
                                    handle_other_messages(
                                        robot_ref_c.clone(),
                                        my_id.clone(),
                                        message,
                                    );
                                });
                            }
                        }
                        read_buffer.clear(); // Clear the buffer
                    }
                    None => {
                        //
                    }
                }
            }
            Err(e) => {
                // Murio el robot donde yo estaba leyendo mensajes.
                let prev_robot_id = robot_ref_c.find_prev_robot(my_id);
                // prev robot id es el que murio. Tengo que notificar su baja.
                if let Some(dead_id) = prev_robot_id {
                    println!("ID Robot Muerto: {}", dead_id);
                    if robot.is_net_size_2() {
                        // si el tamano de la red era 2 y murio el robot, estoy solo.
                        // update network
                        Robot::update_network(robot.clone(), dead_id);

                        // kill old sender thread
                        let _ = tx_sender.send(MessageType::Kill());

                        // start another sender thread
                        let robot_ref = robot.clone();
                        thread::spawn(move || {
                            robot_ref.connect_to_next_robot();
                        });

                        if robot.is_leader(my_id) {
                        } else {
                            robot.set_new_leader(my_id, my_id);
                            // screen sender thread start todo!()
                        }

                        for flavor in IceCreamFlavor::iter() {
                            // para cada gusto. Me fijo si se perdio el token.
                            if robot.has_token(flavor) {
                                //println!("Yo tengo el token de {:?}, no se perdio...", flavor);
                                // Yo tengo el token, no se perdio.
                            } else {
                                // Emitir token perdido.
                                println!("No se encontro el token de {:?}", flavor);
                                let current_stock = robot.get_flavor_stock(flavor);
                                let lost_flavour_token = MessageType::Token(Token {
                                    sender_id: my_id,
                                    flavour: flavor.clone(),
                                    last_modified_by_id: my_id,
                                    last_modification_timestamp: SystemTime::now(),
                                    available_ammount: current_stock.clone(),
                                });
                                // println!(
                                //     "Nuevo token de {:?} emitido con stock: {:?}",
                                //     flavor, current_stock
                                // );
                                let _ = tx_sender.send(lost_flavour_token);
                            }
                        }

                        // Buscar ordenes en la tabla de robot muerto.
                        let lost_orders = robot.get_robot_orders(dead_id);
                        println!(
                            "Las siguientes ordenes las estaba laburando el robot muerto: {:?},",
                            lost_orders
                        );

                        robot.remove_dead_from_orders_table(dead_id);
                        // Stock recuperado por los pedidos perdidos.
                        recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);
                        // Para los pedidos perdidos, los intento re asignar.
                        reassign_lost_orders(lost_orders, &robot, my_id);

                        // Inicio screen sender thread con pantalla
                        let arc_robot = robot.clone();
                        thread::spawn(move || arc_robot.connect_to_screen(true));
                    } else {
                        // Notificar al siguiente robot.
                        let deadrobot_msg = MessageType::DeadRobot(DeadRobot {
                            sender_id: my_id,
                            dead_robot_id: dead_id,
                        });
                        let _ = tx_sender.send(deadrobot_msg);

                        // Si el id muerto es el lider, hay que elegir nuevo lider
                        if robot_ref.is_leader(dead_id) {
                            robot_ref.send_leader_election_message(my_id, dead_id);
                        }

                        // Update network
                        Robot::update_network(robot.clone(), dead_id);
                    }

                    // Si soy el lider. Revisar si se perdieron tokens.
                    if robot.is_leader(my_id) {
                        println!("Robot muerto detectado. Ver si se perdieron tokens.");
                        recover_lost_tokens(&robot, &tx_sender);

                        // Soy lider, tengo que re asignar pedidos que pertenecian al robot muerto.
                        // buscar ordenes en la tabla de ese robot.
                        let lost_orders = robot.get_robot_orders(dead_id);
                        println!(
                            "Las siguientes ordenes correspondian al robot muerto: {:?},",
                            lost_orders
                        );

                        robot.remove_dead_from_orders_table(dead_id);
                        let remove_robot_msg = MessageType::RemoveRobot(dead_id.clone());
                        let _ = tx_sender.send(remove_robot_msg);

                        // Stock recuperado por los pedidos perdidos.
                        recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);

                        // Para los pedidos perdidos, los intento re asignar.
                        reassign_lost_orders(lost_orders, &robot, my_id);
                    }
                    break;
                } else {
                    eprintln!("No Value Found");
                }
                eprintln!("Robot {}: Failed to read from socket; err = {:?}", my_id, e);
                break;
            }
        }
    }
}

/// Función para procesar mensajes que no son de tipo "Prepare" provenientes de nodos robot.
///
/// Esta función maneja varios tipos de mensajes recibidos de la red.
/// Dependiendo del tipo de mensaje, puede involucrar acciones
/// como la detección de nodos caídos, la emisión de nuevos tokens, la gestión de órdenes,
/// y la actualización del estado interno de la red de robots.
///
pub fn handle_other_messages(robot: Arc<Robot>, my_id: usize, message: MessageType) {
    let tx_sender = robot.tx_sender_channel.clone();
    match message {
        MessageType::DeadRobot(ref dead_robot_data) => {
            let dead_robot_id = dead_robot_data.dead_robot_id;

            if robot.is_connected_to_me(dead_robot_id) {
                // si el robot que murio es al que yo le enviaba mensajes, matar el sender thread.
                let _ = tx_sender.send(MessageType::Kill());

                // update network
                Robot::update_network(robot.clone(), dead_robot_id);

                // start another sender thread
                let robot_ref = robot.clone();
                thread::spawn(move || {
                    robot_ref.connect_to_next_robot();
                });
            } else {
                // Update network
                Robot::update_network(robot.clone(), dead_robot_id);
                // Forward the message
                let _ = tx_sender.send(message);
            }

            robot.print_network(); // debug print

            // Soy lider y se detecto la caida de un robot:
            if robot.is_leader(my_id) {
                // Ver si se perdieron tokens.
                println!("Robot muerto detectado. Ver si se perdieron tokens.");
                recover_lost_tokens(&robot, &tx_sender);

                // Soy lider, tengo que re asignar pedidos que pertenecian al robot muerto.
                // buscar ordenes en la tabla de ese robot.
                let lost_orders = robot.get_robot_orders(dead_robot_id);
                println!(
                    "Las siguientes ordenes correspondian al robot muerto: {:?},",
                    lost_orders
                );

                robot.remove_dead_from_orders_table(dead_robot_id);
                let remove_robot_msg = MessageType::RemoveRobot(dead_robot_id.clone());
                let _ = tx_sender.send(remove_robot_msg);

                // Stock recuperado por los pedidos perdidos.
                recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);
                reassign_lost_orders(lost_orders, &robot, my_id);
            }
        }
        MessageType::PossibleLostToken(ref lost_token_data) => {
            // Si soy el lider y recibi este mensaje. Significa que se perdio el token.
            // Emito uno nuevo usando la ultima data disponible.

            //println!("Recibido possible lost token: {:?}", lost_token_data);
            if robot.is_leader(my_id) {
                println!("No se encontro el token de {:?}", lost_token_data.flavor);
                let lost_flavour_token = MessageType::Token(Token {
                    sender_id: my_id,
                    flavour: lost_token_data.flavor.clone(),
                    last_modified_by_id: my_id,
                    last_modification_timestamp: lost_token_data.timestamp,
                    available_ammount: lost_token_data.stock,
                });
                // println!(
                //     "Nuevo token de {:?} emitido con stock: {:?}",
                //     lost_token_data.flavor, lost_token_data.stock
                // );
                let _ = tx_sender.send(lost_flavour_token);
            } else {
                //println!("No soy  lider, me fijo si tengo el token");
                if robot.has_token(&lost_token_data.flavor) {
                    println!(
                        "Yo tengo el token de {:?}, no se perdio.",
                        lost_token_data.flavor
                    );
                    let found_token_msg = MessageType::TokenFound(lost_token_data.flavor.clone());
                    let _ = tx_sender.send(found_token_msg);

                    // si la ultima vez que modifique (tuve acceso al token) es mayor
                    // al timestamp donde se supone perdido, entonces no se perdio el token.
                } else if robot
                    .is_timestamp_greater(&lost_token_data.flavor, lost_token_data.timestamp)
                {
                    let found_token_msg = MessageType::TokenFound(lost_token_data.flavor.clone());
                    let _ = tx_sender.send(found_token_msg);
                } else {
                    //println!("bueno no encontre el token. Forwardeo mensaje de lost");
                    let _ = tx_sender.send(message);
                }
            }
        }
        MessageType::TokenFound(ref flavor) => {
            if robot.is_leader(my_id) {
                // do nothing
                println!("Encontrado el token de: {:?}, no se perdio!", flavor);
            } else {
                let _ = tx_sender.send(message);
            }
        }
        MessageType::NewOrder(ref order_data) => {
            if robot.is_leader(my_id) {
                // Do nothing
            } else {
                // Update internal orders table
                robot.add_new_order(
                    order_data.target_id,
                    order_data.order_id,
                    order_data.order_details.clone(),
                );

                // Forward the message
                let _ = tx_sender.send(message);
            }
        }
        MessageType::RemoveRobot(ref dead_id) => {
            if robot.is_leader(my_id) {
                // do nothing
            } else {
                println!("Murio el robot {}, lo saco de la tabla interna", dead_id);
                robot.remove_dead_from_orders_table(*dead_id);
                let _ = tx_sender.send(message);
            }
        }
        MessageType::OrderComplete(ref order_data) => {
            // si soy lider tengo que:
            if robot.is_leader(my_id) {
                // notificar a la pantalla --> (armar mensaje)
                let commit_msg = MessageType::Commit(Commit {
                    order_id: order_data.order_id,
                });
                let _ = robot.tx_screen_sender_channel.send(commit_msg);

                // actualizar tabla interna (borrar el pedido completado)
                robot.remove_completed_order(order_data.robot_id_maker, order_data.order_id);

                // mandar mensaje de order delivered a los robots.
                let ordered_delivered_msg = MessageType::OrderDelivered(OrderDelivered {
                    robot_id_maker: order_data.robot_id_maker,
                    order_id: order_data.order_id,
                });
                let _ = tx_sender.send(ordered_delivered_msg);
            } else {
                // forward the message
                let _ = tx_sender.send(message);
            }
        }
        MessageType::OrderDelivered(ref order_data) => {
            if robot.is_leader(my_id) {
                // do nothing
            } else {
                // actualizar tabla interna (borrar pedido completado)
                robot.remove_completed_order(order_data.robot_id_maker, order_data.order_id);
                // mandar mensaje
                let _ = tx_sender.send(message);
            }
        }
        MessageType::UpdateStock(ref update_data) => {
            if robot.is_leader(my_id) {
                // do nothing
            } else {
                // actualizar tabla de stock interna
                if update_data.subtract {
                    robot.subtract_stock_with_timestamp(
                        update_data.modified_values.clone(),
                        update_data.timestamp,
                    );
                } else {
                    robot.add_stock_with_timestamp(
                        update_data.modified_values.clone(),
                        update_data.timestamp,
                    );
                }

                // mandar mensaje al anillo
                let _ = tx_sender.send(message);
            }
        }
        MessageType::NewLeader(ref new_leader_data) => {
            let new_leader_id = new_leader_data.new_leader_id;

            if new_leader_id != my_id {
                // forward the message
                robot.set_new_leader(my_id, new_leader_id);
                let _ = tx_sender.send(message);
            }
        }
        MessageType::Election(ref election_data) => {
            let current_candidate_id = election_data.current_candidate_id;

            if current_candidate_id == my_id {
                // Soy el nuevo lider de la red.
                robot.set_new_leader(my_id, current_candidate_id);

                // Avisar al resto de la red el nuevo lider.
                let newleader_msg = MessageType::NewLeader(NewLeader {
                    sender_id: my_id,
                    new_leader_id: my_id,
                    dead_leader_id: election_data.dead_leader_id,
                });
                let _ = tx_sender.send(newleader_msg);

                let dead_id = election_data.dead_leader_id;
                //Soy el nuevo lder. Reemplazo al lider "dead_leader_id".

                // Ver si se perdieron tokens.
                println!("Soy el nuevo lider y murio un robot (viejo lider)... a ver si se perdieron tokens");
                recover_lost_tokens(&robot, &tx_sender);

                // Soy lider, tengo que re asignar pedidos que pertenecian al robot muerto.
                // buscar ordenes en la tabla de ese robot.
                let lost_orders = robot.get_robot_orders(dead_id);
                println!(
                    "Las siguientes ordenes las estaba laburando el robot muerto: {:?},",
                    lost_orders
                );

                robot.remove_dead_from_orders_table(dead_id);
                let remove_robot_msg = MessageType::RemoveRobot(dead_id.clone());
                let _ = tx_sender.send(remove_robot_msg);

                // Stock recuperado por los pedidos perdidos.
                recover_stock_from_lost_orders(&lost_orders, &robot, &tx_sender);
                // Para los pedidos perdidos, los intento re asignar.
                reassign_lost_orders(lost_orders, &robot, my_id);

                // Inicio screen sender thread con pantalla
                let arc_robot = robot.clone();
                thread::spawn(move || arc_robot.connect_to_screen(true));
            } else if current_candidate_id > my_id {
                // Se elije al robot con menor id para que sea lider
                // Pisar mensaje con mi id y mandar
                let election_msg = MessageType::Election(Election {
                    sender_id: my_id,
                    current_candidate_id: my_id,
                    dead_leader_id: election_data.dead_leader_id,
                });

                let _ = tx_sender.send(election_msg);
            } else {
                // Mandar el mensaje sin cambios
                let _ = tx_sender.send(message);
            }
        }
        MessageType::UpdateScreenLeader(ref new_leader_id) => {
            if robot.is_leader(my_id) {
                //
            } else {
                robot.set_screen_leader(*new_leader_id);
                let _ = robot.tx_sender_channel.send(message);
            }
        }
        MessageType::AllConnected(_) => {
            if my_id == 0 {
                let arc_robot = robot.clone();

                thread::spawn(move || {
                    arc_robot.connect_to_next_robot();
                });

                robot.initialize_tokens();
            } else {
                // Forward the message to the next robot.
                let _ = tx_sender.send(message);
            }
        }
        MessageType::Token(ref token_data) => {
            // Recibido mensaje de Token
            robot.set_token_status(token_data.flavour.clone(), true);
            let _ = robot.tx_token_channel.send(message);
        }
        _ => {
            println!("Unknown: {:?}", message);
        }
    }
}

/// Recupera tokens perdidos verificando cada sabor y enviando mensajes de posible pérdida si es necesario.
///
/// Esta función revisa cada tipo de sabor de helado para verificar si el robot actual tiene el token correspondiente.
/// Si no lo tiene, envía un mensaje de posible pérdida a través del canal de comunicación especificado.
fn recover_lost_tokens(
    robot: &Arc<Robot>,
    tx_sender: &Arc<crossbeam_channel::Sender<MessageType>>,
) {
    let now = SystemTime::now();
    for flavor in IceCreamFlavor::iter() {
        if robot.has_token(flavor) {
            // Yo tengo el token, no se perdio.
        } else {
            println!(
                "Yo no tengo el token de {:?}, mando msj posible lost",
                flavor
            );
            let lost_token_msg = MessageType::PossibleLostToken(TokenData {
                flavor: flavor.clone(),
                timestamp: now,
                stock: robot.get_flavor_stock(flavor),
            });

            let _ = tx_sender.send(lost_token_msg);
        }
    }
}

/// Recupera el stock perdido basado en las órdenes perdidas del robot muerto,
/// actualizando el stock interno y notificando a otros robots.
fn recover_stock_from_lost_orders(
    lost_orders: &Option<crate::robot::robot_orders_table::OrdersList>,
    robot: &Arc<Robot>,
    tx_sender: &Arc<crossbeam_channel::Sender<MessageType>>,
) {
    match *lost_orders {
        Some(ref orders_list) => {
            for (i, order) in orders_list.orders.iter().enumerate() {
                let flavor_map = &order.order_details;
                if i == 0 {
                    // For the first order
                    robot.subtract_stock(flavor_map.clone());
                    let update_stock_msg = MessageType::UpdateStock(UpdateData {
                        modified_values: flavor_map.clone(),
                        timestamp: SystemTime::now(),
                        subtract: true,
                    });

                    // notifico al resto de los robots para que actualicen su stock
                    let _ = tx_sender.send(update_stock_msg);
                } else {
                    // For the rest of the orders
                    robot.add_stock(flavor_map.clone());
                    let update_stock_msg = MessageType::UpdateStock(UpdateData {
                        modified_values: flavor_map.clone(),
                        timestamp: SystemTime::now(),
                        subtract: false,
                    });

                    // notifico al resto de los robots para que actualicen su stock
                    let _ = tx_sender.send(update_stock_msg);
                }
            }
        }
        None => {
            // No lost orders
        }
    }
}

/// Reasigna órdenes perdidas a robots disponibles o al propio robot si no hay otros disponibles.
///
/// Esta función toma la lista de órdenes perdidas y las asigna a nuevos robots objetivo disponibles.
/// Si no hay robots disponibles, la orden se asigna al propio robot. En cada caso, se actualiza la tabla
/// interna de órdenes y se envían mensajes `NewOrder` y `Prepare` para informar y procesar las órdenes.
fn reassign_lost_orders(
    lost_orders: Option<crate::robot::robot_orders_table::OrdersList>,
    robot: &Arc<Robot>,
    my_id: usize,
) {
    match lost_orders {
        Some(orders_list) => {
            for order in &orders_list.orders {
                let order_id = order.order_id;
                let flavor_map = &order.order_details;
                // Para cada orden perdida, asignamos un nuevo target robot

                if robot.has_enough_stock(flavor_map.clone()) {
                    if let Some(target_id) = robot.find_target_robot() {
                        println!(
                            "Orden id {} asignada al robot id: {}",
                            order.order_id, target_id
                        );
                        let prepare_msg = MessageType::Prepare(Prepare {
                            sender_id: my_id,
                            order_id: order_id.clone(),
                            target_id: target_id,
                            order_details: flavor_map.clone(),
                        });

                        // Actualizar tabla interna.
                        robot.add_new_order(target_id, order_id.clone(), flavor_map.clone());

                        // Mandar mensaje NewOrder con detalles.
                        let neworder_msg = MessageType::NewOrder(OrderData {
                            target_id: target_id,
                            order_id: order_id.clone(),
                            order_details: flavor_map.clone(),
                        });
                        let _ = robot.tx_sender_channel.send(neworder_msg);

                        // Mandar mensaje de Prepare
                        let _ = robot.tx_sender_channel.send(prepare_msg);
                    } else {
                        // No encontre target robot -> me lo asigno a mi mismo

                        robot.add_new_order(my_id, order_id.clone(), flavor_map.clone());

                        // mandar mensaje NewOrder para que el resto de los robots actualicen tabla de pedidos.
                        let neworder_msg = MessageType::NewOrder(OrderData {
                            target_id: my_id,
                            order_id: order_id.clone(),
                            order_details: flavor_map.clone(),
                        });
                        let _ = robot.tx_sender_channel.send(neworder_msg);

                        // Preparo la orden
                        let prepare_msg = MessageType::Prepare(Prepare {
                            sender_id: my_id,
                            order_id: order_id.clone(),
                            target_id: my_id,
                            order_details: flavor_map.clone(),
                        });
                        let _ = robot.tx_prepare_channel.send(prepare_msg);
                    }
                } else {
                    // Abort order por falta de stock
                    let abort_order_msg = MessageType::Abort(Abort {
                        order_id: order_id.clone(),
                    });

                    let _ = robot.tx_screen_sender_channel.send(abort_order_msg);
                }
            }
        }
        None => {
            println!("No lost orders.");
        }
    }
}

/// Maneja la conexión de una pantalla (screen) a través de una conexión TCP.
///
/// Esta función establece el líder de la pantalla, maneja la conexión y desconexión de la pantalla,
/// y procesa mensajes de tipo `Order` recibidos desde la pantalla. Si el robot es líder, acepta
/// y procesa pedidos de acuerdo al stock disponible y asigna órdenes a robots objetivo disponibles
/// o al propio robot si no hay otros disponibles.
pub fn handle_screen_connection(
    robot: Arc<Robot>,
    my_id: usize,
    mut socket: TcpStream,
    screen_id: usize,
    is_connected: bool,
) {
    robot.set_screen_leader(screen_id);
    let update_screen_leader_msg = MessageType::UpdateScreenLeader(screen_id);
    let _ = robot.tx_sender_channel.send(update_screen_leader_msg);

    // Spawn screen sender thread.
    if !is_connected {
        let arc_robot = robot.clone();
        thread::spawn(move || arc_robot.connect_to_screen(false));
    }

    let tx_sender_robot = robot.tx_sender_channel.clone();
    let tx_sender_screen = robot.tx_screen_sender_channel.clone();
    let tx_prepare = robot.tx_prepare_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer: Vec<u8> = Vec::new();

    loop {
        match socket.read(&mut buffer) {
            Ok(0) => {
                let kill_msg = MessageType::Kill();
                let _ = tx_sender_screen.send(kill_msg);
                break;
            }
            Ok(n) => {
                read_buffer.extend_from_slice(&buffer[..n]);
                match deserialize_message(&buffer) {
                    Some(message) => {
                        match message {
                            MessageType::Order(ref order_data) => {
                                if robot.is_leader(my_id) {
                                    if robot.has_enough_stock(order_data.order_details.clone()) {
                                        let update_timestamp =
                                            robot.subtract_stock(order_data.order_details.clone());

                                        let update_stock_msg =
                                            MessageType::UpdateStock(UpdateData {
                                                modified_values: order_data.order_details.clone(),
                                                timestamp: update_timestamp,
                                                subtract: true,
                                            });

                                        let _ = tx_sender_robot.send(update_stock_msg);

                                        if let Some(target_id) = robot.find_target_robot() {
                                            println!(
                                                "Orden id {} asignada al robot id: {}",
                                                order_data.order_id.clone(),
                                                target_id
                                            );
                                            let prepare_msg = MessageType::Prepare(Prepare {
                                                sender_id: my_id,
                                                order_id: order_data.order_id.clone(),
                                                target_id: target_id,
                                                order_details: order_data.order_details.clone(),
                                            });

                                            robot.add_new_order(
                                                target_id,
                                                order_data.order_id,
                                                order_data.order_details.clone(),
                                            );

                                            let neworder_msg = MessageType::NewOrder(OrderData {
                                                target_id: target_id,
                                                order_id: order_data.order_id,
                                                order_details: order_data.order_details.clone(),
                                            });
                                            let _ = tx_sender_robot.send(neworder_msg);

                                            let _ = tx_sender_robot.send(prepare_msg);
                                        } else {
                                            robot.add_new_order(
                                                my_id,
                                                order_data.order_id,
                                                order_data.order_details.clone(),
                                            );

                                            let neworder_msg = MessageType::NewOrder(OrderData {
                                                target_id: my_id,
                                                order_id: order_data.order_id,
                                                order_details: order_data.order_details.clone(),
                                            });
                                            let _ = tx_sender_robot.send(neworder_msg);

                                            let prepare_msg = MessageType::Prepare(Prepare {
                                                sender_id: my_id,
                                                order_id: order_data.order_id.clone(),
                                                target_id: my_id,
                                                order_details: order_data.order_details.clone(),
                                            });
                                            let _ = tx_prepare.send(prepare_msg);
                                        }
                                    } else {
                                        let abort_order_msg = MessageType::Abort(Abort {
                                            order_id: order_data.order_id,
                                        });

                                        let _ = tx_sender_screen.send(abort_order_msg);
                                    }
                                }
                            }
                            _ => {
                                //eprintln!("Unknown message type received from Screen, {:?}",message);
                            }
                        }
                        read_buffer.clear(); // Clear the buffer
                    }
                    None => {
                        //
                    }
                }
            }

            Err(_) => {
                //println!("Screen leader murio");
                let kill_msg = MessageType::Kill();
                let _ = tx_sender_screen.send(kill_msg);
                break;
            }
        }
    }
}
