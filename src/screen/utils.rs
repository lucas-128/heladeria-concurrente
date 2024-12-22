// use crate::screen::Screen;
// use heladeria::common::constants::BUFFER_SIZE;
// use heladeria::common::messages::*;
use std::{io::Read, net::TcpStream, sync::Arc, thread};

use crate::common::{constants::BUFFER_SIZE, messages::*};

use super::screen::Screen;

/// Maneja una conexión TCP entrante, esperando un mensaje de introducción que determina
/// si el nodo conectado es un robot o una pantalla. Según el tipo de nodo,
/// llama a funciones específicas para manejar mensajes entrantes adicionales.
pub fn handle_incoming_connection(screen: Arc<Screen>, mut socket: TcpStream, my_id: usize) {
    let screen_ref = screen.clone();
    let _tx_sender = screen.tx_sender_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer = Vec::new();

    let screen_ref_c = screen_ref.clone();
    match socket.read(&mut buffer) {
        Ok(0) => {
            //Connection closed
        }
        Ok(n) => {
            read_buffer.extend_from_slice(&buffer[..n]);
            match deserialize_message(&buffer) {
                Some(message) => {
                    match message {
                        MessageType::ScreenIntroduction(_) => {
                            handle_screen_connection(
                                screen_ref_c,
                                my_id,
                                socket.try_clone().unwrap(),
                            );
                        }
                        MessageType::RobotIntroduction(_) => {
                            handle_robot_connection(
                                screen_ref_c,
                                my_id,
                                socket.try_clone().unwrap(),
                            );
                        }
                        MessageType::NewLeaderIntroduction(robot_id) => {
                            // set new robot leader id y mandar a todos para que sepan
                            screen_ref_c.set_new_robot_leader(robot_id);
                            let new_robot_leader_msg =
                                MessageType::UpdateRobotLeader(robot_id.clone());
                            let _ = screen_ref_c.tx_sender_channel.send(new_robot_leader_msg);

                            // new sender thread con el nuevo robot lider
                            let screen_arc = screen_ref_c.clone();
                            screen_arc.connect_robot(true);

                            handle_robot_connection(
                                screen_ref_c,
                                my_id,
                                socket.try_clone().unwrap(),
                            );
                        }
                        _ => {
                            println!("Mensaje no manejado: {:?}", message);
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
            eprintln!(
                "screen {}: Failed to read from socket; err = {:?}",
                my_id, e
            );
        }
    }
}

/// Maneja la conexión de un nodo screen a través de una conexión TCP.
///
/// Esta función lee mensajes desde el socket y los procesa según su tipo.
///
pub fn handle_screen_connection(screen: Arc<Screen>, my_id: usize, mut socket: TcpStream) {
    let screen_ref = screen.clone();
    let tx_sender = screen.tx_sender_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer = Vec::new();

    loop {
        let screen_ref_c = screen_ref.clone();
        match socket.read(&mut buffer) {
            Ok(0) => {
                let prev_screen_id = screen_ref_c.find_prev_screen(my_id);
                // prev screen id es el que murio. Tengo que notificar su baja.
                if let Some(dead_id) = prev_screen_id {
                    println!("ID screen Muerto: {}", dead_id);
                    screen.transfer_orders(dead_id, my_id);
                    let deadscreen_msg = MessageType::DeadScreen(DeadScreen {
                        sender_id: my_id,
                        dead_screen_id: dead_id,
                    });

                    let _ = tx_sender.send(deadscreen_msg);

                    // update network
                    Screen::update_network(screen.clone(), dead_id);

                    // Si el id muerto es el lider, hay que elegir nuevo lider
                    if screen_ref.is_leader(dead_id) {
                        screen_ref.send_leader_election_message(my_id, dead_id);
                    }

                    break;
                }
            }
            Ok(n) => {
                read_buffer.extend_from_slice(&buffer[..n]);
                match deserialize_message(&buffer) {
                    Some(message) => {
                        match message {
                            MessageType::OrderScreen(ref order) => {
                                if order.sender_id != my_id {
                                    //println!("Recibido Order: {:?}", order);
                                    Screen::apply_order(screen.clone(), order);
                                    tx_sender.send(message).unwrap();
                                } else {
                                    Screen::apply_order(screen.clone(), order);
                                }
                            }

                            // Other type of message
                            _ => {
                                thread::spawn(move || {
                                    handle_other_messages(
                                        screen_ref_c.clone(),
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
                let prev_screen_id = screen_ref_c.find_prev_screen(my_id);
                // prev screen id es el que murio. Tengo que notificar su baja.
                if let Some(dead_id) = prev_screen_id {
                    println!("ID screen Muerto: {}", dead_id);
                    screen.transfer_orders(dead_id, my_id);
                    let deadscreen_msg = MessageType::DeadScreen(DeadScreen {
                        sender_id: my_id,
                        dead_screen_id: dead_id,
                    });

                    let _ = tx_sender.send(deadscreen_msg);

                    // update network
                    Screen::update_network(screen.clone(), dead_id);

                    // Si el id muerto es el lider, hay que elegir nuevo lider
                    if screen_ref.is_leader(dead_id) {
                        screen_ref.send_leader_election_message(my_id, dead_id);
                    }

                    break;
                } else {
                    eprintln!("No Value Found");
                }
                eprintln!(
                    "screen {}: Failed to read from socket; err = {:?}",
                    my_id, e
                );
                break;
            }
        }
    }
}

/// Maneja los mensajes entrantes de un nodo robot a través de una conexión TCP.
///
/// Esta función lee mensajes desde el socket y los procesa según su tipo.
///
pub fn handle_robot_connection(screen: Arc<Screen>, _my_id: usize, mut socket: TcpStream) {
    let screen_ref = screen.clone();
    //let tx_sender = screen.tx_robot_sender_channel.clone();
    let tx_prepare = screen.tx_prepare_channel.clone();
    let tx_ring_sender = screen.tx_sender_channel.clone();
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut read_buffer = Vec::new();
    loop {
        let _screen_ref_c = screen_ref.clone();
        match socket.read(&mut buffer) {
            Ok(0) => {
                let _ = tx_prepare.send(MessageType::Kill());
                break;
            }
            Ok(n) => {
                read_buffer.extend_from_slice(&buffer[..n]);
                match deserialize_message(&buffer) {
                    Some(message) => {
                        match message {
                            MessageType::Commit(ref commit) => {
                                Screen::commit_order(screen.clone(), commit);
                                tx_ring_sender.send(message).unwrap();
                            }
                            MessageType::Abort(ref abort) => {
                                Screen::abort_order(screen.clone(), abort);
                                tx_ring_sender.send(message).unwrap();
                            }
                            // Other type of message
                            _ => {}
                        }
                        read_buffer.clear(); // Clear the buffer
                    }
                    None => {
                        //
                    }
                }
            }

            Err(_e) => {
                // Mato la conexion y mando mensaje para que se de cuenta
                // de que se desconectó el socket
                let _ = tx_prepare.send(MessageType::Kill());
                break;
            }
        }
    }
}

/// Función para procesar mensajes que no son de tipo "Order" provenientes de nodos screen.
///
/// Esta función maneja varios tipos de mensajes recibidos de la red.
/// Dependiendo del tipo de mensaje, puede involucrar acciones
/// como la detección de nodos caídos, eleccion de un nuevo lider y la confirmacion o rechazo de ordenes.
///
pub fn handle_other_messages(screen: Arc<Screen>, my_id: usize, message: MessageType) {
    let tx_sender = screen.tx_sender_channel.clone();

    match message {
        MessageType::DeadScreen(ref dead_screen_data) => {
            let dead_screen_id = dead_screen_data.dead_screen_id;
            screen.transfer_orders(dead_screen_id, dead_screen_data.sender_id);
            if screen.is_connected_to_me(dead_screen_id) {
                // si el screen que murio es al que yo le enviaba mensajes, liquidar el sender thread.
                tx_sender.send(MessageType::Kill()).unwrap();

                // update network
                Screen::update_network(screen.clone(), dead_screen_id);

                // connect to new screen (nuevo sender thread)
                //screen.clone().introduce_myself();
                let screen_intro_msg =
                    MessageType::ScreenIntroduction(ScreenIntroduction { sender_id: my_id });

                tx_sender.send(screen_intro_msg).unwrap();
                screen.connect_to_next_screen();
            } else {
                // update network
                Screen::update_network(screen.clone(), dead_screen_id);
                // Forward the message
                tx_sender.send(message).unwrap();
            }
            screen.print_network(); // debug print
        }
        MessageType::NewLeader(ref new_leader_data) => {
            let new_leader_id = new_leader_data.new_leader_id;

            if new_leader_id != my_id {
                // forward the message
                screen.set_new_leader(my_id, new_leader_id);
                tx_sender.send(message).unwrap();
            }
        }
        MessageType::UpdateRobotLeader(ref new_robot_leader_id) => {
            if screen.i_am_leader() {
                // do nothing
            } else {
                screen.set_new_robot_leader(*new_robot_leader_id);
                let _ = tx_sender.send(message);
            }
        }
        MessageType::Election(ref election_data) => {
            let current_candidate_id = election_data.current_candidate_id;

            if current_candidate_id == my_id {
                // Soy el lider de la red.
                screen.set_new_leader(my_id, current_candidate_id);
                screen.connect_robot(false);

                // Avisar al resto de la red el nuevo lider.
                let newleader_msg = MessageType::NewLeader(NewLeader {
                    sender_id: my_id,
                    new_leader_id: my_id,
                    dead_leader_id: election_data.dead_leader_id,
                });

                let _ = tx_sender.send(newleader_msg);
            } else if current_candidate_id > my_id {
                // Pisar mensaje con mi id y mandar
                let election_msg = MessageType::Election(Election {
                    sender_id: my_id,
                    current_candidate_id: my_id,
                    dead_leader_id: election_data.dead_leader_id,
                });

                tx_sender.send(election_msg).unwrap();
            } else {
                // Mandar el mensaje sin cambios
                tx_sender.send(message).unwrap();
            }
        }
        MessageType::AllConnected(_) => {
            if my_id == 0 {
                //screen.clone().introduce_myself();
                let screen_intro_msg =
                    MessageType::ScreenIntroduction(ScreenIntroduction { sender_id: my_id });

                tx_sender.send(screen_intro_msg).unwrap();
                screen.clone().start_orders(screen.get_robot_channel());
                screen.clone().connect_robot(false);
                screen.connect_to_next_screen();
            } else {
                // Forward the message to the next screen.
                tx_sender.send(message).unwrap();
                screen.clone().start_orders(screen.get_robot_channel());
            }
        }
        MessageType::Commit(ref commit) => {
            if !screen.i_am_leader() {
                Screen::commit_order(screen.clone(), commit);
                tx_sender.send(message).unwrap();
            }
        }
        MessageType::Abort(ref abort) => {
            if !screen.i_am_leader() {
                Screen::abort_order(screen.clone(), abort);
                tx_sender.send(message).unwrap();
            }
        }
        _ => {
            println!("Recibi otra cosa... {:?}", message);
        }
    }
}
