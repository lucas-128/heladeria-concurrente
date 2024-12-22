use crate::fs::remove_file;
use serial_test::serial;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
//cargo test -- --nocapture

const LOG_FILE_PATH: &str = "transactions.log";
const ORDERS_TEST: &str = "orders_test.json";

#[cfg(test)]
mod test {

    use super::*;

    use std::process::{Child, Command};

    fn start_gateway(reject_percentage: Option<u8>) -> Child {
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--bin", "gateway"]);
        if let Some(percentage) = reject_percentage {
            cmd.arg(&percentage.to_string());
        }
        cmd.spawn().expect("Failed to start gateway")
    }

    fn start_robot(robot_id: u8, total_robots: u8) -> Child {
        Command::new("cargo")
            .args(&[
                "run",
                "--bin",
                "robot",
                &robot_id.to_string(),
                &total_robots.to_string(),
            ])
            .spawn()
            .expect("Failed to start robot")
    }

    fn start_screen(screen_id: u8, total_screens: u8, file_name: &str) -> Child {
        Command::new("cargo")
            .args(&[
                "run",
                "--bin",
                "screen",
                &screen_id.to_string(),
                &total_screens.to_string(),
                file_name,
            ])
            .spawn()
            .expect("Failed to start screen")
    }

    fn create_rejected_flavour_test_orders() {
        let orders = r#"
        {
    "orders": [
        {
            "flavors": [
                {
                    "name": "Banana Split",
                    "grams": 100
                }
            ],
            "total_grams": 100
        }
    ]
}
        "#;

        let mut file = File::create(ORDERS_TEST).expect("Failed to create test orders file");
        file.write_all(orders.as_bytes())
            .expect("Failed to write to test orders file");
    }

    fn create_test_orders() {
        let orders = r#"
        {
    "orders": [
        {
            "flavors": [
                {
                    "name": "Mint",
                    "grams": 100
                },
                {
                    "name": "Vanilla",
                    "grams": 150
                }
            ],
            "total_grams": 250
        },
        {
            "flavors": [
                {
                    "name": "Mint",
                    "grams": 200
                },
                {
                    "name": "Vanilla",
                    "grams": 150
                },
                {
                    "name": "Chocolate",
                    "grams": 150
                }
            ],
            "total_grams": 500
        }
    ]
}
        "#;

        let mut file = File::create(ORDERS_TEST).expect("Failed to create test orders file");
        file.write_all(orders.as_bytes())
            .expect("Failed to write to test orders file");
    }

    fn create_over_stock_orders() {
        let orders = r#"
        {
          "orders": [
            {
              "flavors": [
                {
                  "name": "Mint",
                  "grams": 10001
                }
              ],
              "total_grams": 10001
            }
          ]
        }
        "#;

        let mut file = File::create(ORDERS_TEST).expect("Failed to create test orders file");
        file.write_all(orders.as_bytes())
            .expect("Failed to write to test orders file");
    }

    fn create_full_stock_orders() {
        let orders = r#"
    {
      "orders": [
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        },
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        },
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        },
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        },
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        },
        {
          "flavors": [
            {
              "name": "Mint",
              "grams": 1000
            }
          ],
          "total_grams": 1000
        }
      ]
    }
    "#;

        let mut file = File::create(ORDERS_TEST).expect("Failed to create test orders file");
        file.write_all(orders.as_bytes())
            .expect("Failed to write to test orders file");
    }

    fn delete_orders_test_file() {
        if let Err(e) = remove_file(ORDERS_TEST) {
            eprintln!("Failed to delete file {}: {}", ORDERS_TEST, e);
        }
    }

    #[test]
    #[serial]
    fn test_last_aborted_transaction() {
        // Crear archivo JSON con el pedido de chocolate
        create_full_stock_orders();

        // Start gateway
        let mut gateway = start_gateway(Some(0)); // Assuming 100% rejection for simplicity

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(60));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        // Verificar el log para comprobar el estado ABORT
        verify_aborted_transaction(LOG_FILE_PATH);
    }

    #[test]
    #[serial]
    fn test_aborted_transaction() {
        // Crear archivo JSON con el pedido de chocolate
        create_over_stock_orders();

        // Start gateway
        let mut gateway = start_gateway(Some(0)); // Assuming 100% rejection for simplicity

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        // Verificar el log para comprobar el estado ABORT
        verify_aborted_transaction(LOG_FILE_PATH);
    }

    #[test]
    #[serial]
    fn test_rejected_orders() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(100));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_empty_transaction(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_rejected_flavour_orders() {
        // Start gateway
        create_rejected_flavour_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_empty_transaction(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_full_system() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(12));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_screen_leader() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen2 = start_screen(2, 3, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = screen0.kill();

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(12));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot1.kill();
        let _ = robot0.kill();
        let _ = screen2.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_screen() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen2 = start_screen(2, 3, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = screen1.kill();

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(15));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot1.kill();
        let _ = robot0.kill();
        let _ = screen2.kill();
        let _ = screen0.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_robot_leader() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot2 = start_robot(2, 3);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = robot0.kill();

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(12));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = robot2.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_robot() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot2 = start_robot(2, 3);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = robot1.kill();

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(12));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = screen0.kill();
        let _ = robot2.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_robot_and_screen_leader() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot2 = start_robot(2, 3);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen2 = start_screen(2, 3, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = robot0.kill();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let _ = screen0.kill();
        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(15));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot1.kill();
        let _ = screen2.kill();
        let _ = robot2.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_dead_robot_and_screen() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 3);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot2 = start_robot(2, 3);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 3, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen2 = start_screen(2, 3, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = robot1.kill();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let _ = screen1.kill();
        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(15));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot0.kill();
        let _ = screen2.kill();
        let _ = robot2.kill();
        let _ = screen0.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_two_dead_robot() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 4);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 4);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot2 = start_robot(2, 4);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot3 = start_robot(3, 4);
        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 2, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 2, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = robot0.kill();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let _ = robot2.kill();
        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(15));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = robot1.kill();
        let _ = screen0.kill();
        let _ = robot3.kill();
        let _ = screen1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    #[test]
    #[serial]
    fn test_two_dead_screen() {
        // Start gateway
        create_test_orders();
        let mut gateway = start_gateway(Some(0));

        // Start robots
        let mut robot0 = start_robot(0, 2);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut robot1 = start_robot(1, 2);

        // Start screens
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen0 = start_screen(0, 4, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen1 = start_screen(1, 4, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen2 = start_screen(2, 4, ORDERS_TEST);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let mut screen3 = start_screen(3, 4, ORDERS_TEST);

        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(4));

        let _ = screen0.kill();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let _ = screen2.kill();
        // Simulate a waiting period to allow processes to run
        std::thread::sleep(std::time::Duration::from_secs(15));

        // Kill the processes after the test
        let _ = gateway.kill();
        let _ = screen1.kill();
        let _ = robot0.kill();
        let _ = screen3.kill();
        let _ = robot1.kill();
        delete_orders_test_file();
        verify_transactions(LOG_FILE_PATH);
        // Check the outcomes here if necessary
    }

    fn verify_aborted_transaction(log_file: &str) {
        let file = File::open(log_file).expect("Failed to open log file");
        let reader = BufReader::new(file);

        let mut found_abort = false;

        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            if line.starts_with("ABORT") {
                found_abort = true;
                break;
            }
        }

        assert!(found_abort, "No ABORT found");
    }

    fn verify_empty_transaction(log_file: &str) {
        let file = File::open(log_file).expect("Failed to open log file");
        let reader = BufReader::new(file);

        let mut is_empty = true;

        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            if !line.trim().is_empty() {
                is_empty = false;
                break;
            }
        }

        assert!(is_empty, "Transaction log is not empty");
    }

    fn verify_transactions(log_file: &str) {
        let file = File::open(log_file).expect("Failed to open log file");
        let reader = BufReader::new(file);

        let mut prepares = std::collections::HashSet::new();
        let mut commits_or_aborts = std::collections::HashSet::new();

        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            if line.starts_with("PREPARE") {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() > 1 {
                    prepares.insert(parts[1].to_string());
                }
            } else if line.starts_with("COMMIT") || line.starts_with("ABORT") {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() > 1 {
                    commits_or_aborts.insert(parts[1].to_string());
                }
            }
        }

        for prepare_id in &prepares {
            assert!(
                commits_or_aborts.contains(prepare_id),
                "Transaction {} is missing COMMIT or ABORT",
                prepare_id
            );
        }
    }
}
