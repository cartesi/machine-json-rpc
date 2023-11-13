// Copyright (C) 2021 Cartesi Pte. Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cartesi_machine_json_rpc::client::*;
use cartesi_machine_json_rpc::interfaces;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rstest::*;
use std::future::Future;

static INITIAL_ROOT_HASH: [u8; 32] = [
    131, 99, 230, 222, 130, 27, 210, 194, 165, 9, 76, 229, 231, 218, 75, 231, 151, 47, 191, 153,
    22, 84, 94, 122, 127, 184, 180, 51, 152, 100, 117, 189,
];

static SECOND_STEP_HASH: [u8; 32] = [
    234, 235, 196, 19, 105, 77, 70, 208, 245, 14, 10, 142, 22, 194, 160, 55, 13, 172, 207, 44, 129,
    97, 163, 39, 181, 15, 45, 0, 24, 214, 194, 148,
];

#[allow(dead_code)]
struct Context {
    cartesi_machine_server: JsonRpcCartesiMachineClient,
    server_ip: String,
    port: u32,
    container_name: String,
}

fn generate_random_name() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(15)
        .map(char::from)
        .collect()
}

fn instantiate_external_server_instance(port: u32) -> Result<(), Box<dyn std::error::Error>> {
    let address = format!("127.0.0.1:{0}", port);
    let server_address = format!("--server-address=127.0.0.1:{0}", port);

    println!(
        "Starting Cartesi jsonrpc remote machine on address {}",
        address
    );
    match std::process::Command::new("/opt/cartesi/bin/jsonrpc-remote-cartesi-machine")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .arg(server_address)
        .spawn()
    {
        Ok(_child) => {}
        Err(error) => panic!("{}", error.to_string()),
    };
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}

fn try_stop_container() {
    let result = std::process::Command::new("pkill")
        .arg("-f")
        .arg("/usr/bin/jsonrpc-remote-cartesi-machine")
        .status()
        .unwrap();
    if !result.success() {
        eprint!("Error stopping container");
    }
}

impl Context {
    pub fn get_server(&self) -> &JsonRpcCartesiMachineClient {
        &self.cartesi_machine_server
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        println!("Destroying container {}", &self.container_name);
        try_stop_container();
    }
}

#[allow(unused_mut)]
mod local_server {
    use super::*;

    #[fixture]
    async fn context_future() -> Context {
        let server_ip = "127.0.0.1".to_string();
        let port: u32 = rand::thread_rng().gen_range(49152..65535);
        let uri = format!("http://{}:{}", server_ip, port);
        let container_name = generate_random_name();

        match instantiate_external_server_instance(port) {
            Ok(_) => (),
            Err(ex) => eprint!(
                "Error instantiating cartesi machine server {}",
                ex.to_string()
            ),
        }
        println!(
            "Starting jsonrpc machine server: {} server_ip:{}:{} ",
            container_name, server_ip, port
        );

        Context {
            cartesi_machine_server: match JsonRpcCartesiMachineClient::new(uri).await {
                Ok(machine) => machine,
                Err(err) => {
                    panic!("Unable to create machine server: {}", err.to_string())
                }
            },
            port,
            server_ip,
            container_name,
        }
    }

    #[fixture]
    async fn context_with_machine_future() -> Context {
        let server_ip = "127.0.0.1".to_string();
        let port: u32 = rand::thread_rng().gen_range(49152..65535);
        let uri = format!("http://{}:{}", server_ip, port);
        let container_name = generate_random_name();
        match instantiate_external_server_instance(port) {
            Ok(_) => (),
            Err(err) => eprint!(
                "Error instantiating jsonrpc cartesi machine server {}",
                err.to_string()
            ),
        }
        println!(
            "Starting jsonrpc cartesi server: {} server_ip:{}:{} ",
            container_name, server_ip, port
        );
        let context = Context {
            cartesi_machine_server: match JsonRpcCartesiMachineClient::new(uri).await {
                Ok(machine) => machine,
                Err(err) => {
                    panic!(
                        "Unable to create jsonrpc machine server: {}",
                        err.to_string()
                    )
                }
            },
            port,
            server_ip,
            container_name,
        };
        //Modify default configuration
        let mut default_config = match context.get_server().get_default_config().await {
            Ok(config) => config,
            Err(err) => {
                panic!("Unable to get default config: {}", err.to_string())
            }
        };
        default_config.rom = RomConfig {
            bootargs: default_config.rom.bootargs,
            image_filename: String::from("/opt/cartesi/share/images/rom.bin"),
        };
        default_config.ram = RamConfig {
            length: 1 << 20,
            image_filename: String::new(),
        };
        default_config.uarch = UarchConfig {
            processor: Some(interfaces::UarchProcessorConfig {
                x: Some(vec![0; 32]),
                pc: Some(0x70000000),
                cycle: Some(0),
            }),
            ram: Some(interfaces::UarchRAMConfig {
                length: Some(77128),
                image_filename: Some(String::from("/opt/cartesi/share/images/uarch-ram.bin")),
            }),
        };
        default_config.htif = interfaces::HTIFConfig {
            console_getchar: Some(false),
            yield_manual: Some(true),
            yield_automatic: Some(false),
            fromhost: Some(0),
            tohost: Some(0),
        };
        default_config.rollup = RollupConfig {
            input_metadata: Some(MemoryRangeConfig {
                start: 0x60400000,
                length: 4096,
                image_filename: "".to_string(),
                shared: false,
            }),
            notice_hashes: Some(MemoryRangeConfig {
                start: 0x60800000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            rx_buffer: Some(MemoryRangeConfig {
                start: 0x60000000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            voucher_hashes: Some(MemoryRangeConfig {
                start: 0x60600000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            tx_buffer: Some(MemoryRangeConfig {
                start: 0x60200000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
        };

        match context
            .get_server()
            .create_machine(&default_config, &MachineRuntimeConfig::default())
            .await
        {
            Ok(_) => context,
            Err(err) => {
                panic!("Unable to instantiate cartesi machine: {}", err.to_string())
            }
        }
    }

    #[fixture]
    async fn context_with_machine_with_flash_future() -> Context {
        let server_ip = "127.0.0.1".to_string();
        let port: u32 = rand::thread_rng().gen_range(49152..65535);
        let uri = format!("http://{}:{}", server_ip, port);
        let container_name = generate_random_name();
        match instantiate_external_server_instance(port) {
            Ok(_) => (),
            Err(err) => eprint!(
                "Error instantiating jsonrpc cartesi machine server {}",
                err.to_string()
            ),
        }
        println!(
            "Starting jsonrpc cartesi server: {} server_ip:{}:{} ",
            container_name, server_ip, port
        );
        let context = Context {
            cartesi_machine_server: match JsonRpcCartesiMachineClient::new(uri).await {
                Ok(machine) => machine,
                Err(err) => {
                    panic!("Unable to create machine server: {}", err.to_string())
                }
            },
            port,
            server_ip,
            container_name,
        };
        //Modify default configuration
        let mut default_config = match context.get_server().get_default_config().await {
            Ok(config) => config,
            Err(err) => {
                panic!("Unable to get default config: {}", err.to_string())
            }
        };
        default_config.rom = RomConfig {
            bootargs: default_config.rom.bootargs,
            image_filename: String::from("/opt/cartesi/share/images/rom.bin"),
        };
        default_config.ram = RamConfig {
            length: 1 << 20,
            image_filename: String::new(),
        };
        default_config.htif = interfaces::HTIFConfig {
            console_getchar: Some(false),
            yield_manual: Some(true),
            yield_automatic: Some(false),
            fromhost: Some(0),
            tohost: Some(0),
        };
        default_config.rollup = RollupConfig {
            input_metadata: Some(MemoryRangeConfig {
                start: 0x60400000,
                length: 4096,
                image_filename: "".to_string(),
                shared: false,
            }),
            notice_hashes: Some(MemoryRangeConfig {
                start: 0x60800000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            rx_buffer: Some(MemoryRangeConfig {
                start: 0x60000000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            voucher_hashes: Some(MemoryRangeConfig {
                start: 0x60600000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            tx_buffer: Some(MemoryRangeConfig {
                start: 0x60200000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
        };

        //Create flash image and add to flash configuration
        match std::fs::write("/tmp/input_root.raw", b"Root data in flash") {
            Ok(_) => (),
            Err(err) => panic!(
                "Unable to create temporary flash image: {}",
                err.to_string()
            ),
        }
        std::process::Command::new("truncate")
            .args(&["-s", "62914560", "/tmp/input_root.raw"])
            .output()
            .expect("Unable to create flash image file");
        default_config.flash_drives = vec![MemoryRangeConfig {
            start: 0x80000000000000,
            image_filename: "/tmp/input_root.raw".to_string(),
            length: 0x3c00000,
            shared: false,
        }];
        //Create machine
        match context
            .get_server()
            .create_machine(&default_config, &MachineRuntimeConfig::default())
            .await
        {
            Ok(_) => context,
            Err(err) => {
                panic!(
                    "Unable to instantiate jsonrpc cartesi machine: {}",
                    err.to_string()
                )
            }
        }
    }

    #[rstest]
    #[tokio::test]
    #[should_panic]
    async fn test_invalid_server_address() -> () {
        let server_ip = "127.0.0.1".to_string();
        let port: u32 = 12345;
        let uri = format!("http://{}:{}", server_ip, port);
        let container_name = generate_random_name();
        let _context = Context {
            cartesi_machine_server: match JsonRpcCartesiMachineClient::new(uri).await {
                Ok(machine) => machine,
                Err(err) => {
                    panic!("Unable to create machine server: {}", err.to_string())
                }
            },
            port,
            server_ip,
            container_name,
        };
        ()
    }

    #[rstest]
    #[tokio::test]
    async fn test_cartesi_server_instance(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        println!(
            "Sleeping in the test... context container name: {}",
            context.container_name
        );
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("End sleeping");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_version(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let semantic_version = context.get_server().get_version().await?;
        println!("Acquired semantic version: {:?} ", semantic_version);
        assert_eq!(
            semantic_version,
            SemanticVersion {
                major: 0,
                minor: 1,
                patch: 0,
                pre_release: "".to_string(),
                build: "".to_string()
            }
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_default_config(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let default_config = context.get_server().get_default_config().await?;
        println!("Acquired default config {:?}", default_config);
        assert_eq!(default_config.processor.pc, 4096);
        assert_eq!(default_config.processor.mvendorid, 7161130726739634464);
        assert_eq!(default_config.processor.marchid, 0xf);
        assert_eq!(default_config.ram.length, 0);
        assert_eq!(default_config.rom.image_filename, "");
        assert_eq!(default_config.flash_drives.len(), 0);
        assert_eq!(default_config.htif.fromhost, Some(0));
        assert_eq!(default_config.htif.tohost, Some(0));
        assert_eq!(default_config.clint.mtimecmp, Some(0));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_machine_create(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let mut default_config = context.get_server().get_default_config().await?;
        default_config.rom = RomConfig {
            bootargs: default_config.rom.bootargs,
            image_filename: String::from("/opt/cartesi/share/images/rom.bin"),
        };
        default_config.ram = RamConfig {
            length: 1 << 20,
            image_filename: String::new(),
        };
        default_config.htif = interfaces::HTIFConfig {
            console_getchar: Some(false),
            yield_manual: Some(true),
            yield_automatic: Some(false),
            fromhost: Some(0),
            tohost: Some(0),
        };
        default_config.rollup = RollupConfig {
            input_metadata: Some(MemoryRangeConfig {
                start: 0x60400000,
                length: 4096,
                image_filename: "".to_string(),
                shared: false,
            }),
            notice_hashes: Some(MemoryRangeConfig {
                start: 0x60800000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            rx_buffer: Some(MemoryRangeConfig {
                start: 0x60000000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            voucher_hashes: Some(MemoryRangeConfig {
                start: 0x60600000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            tx_buffer: Some(MemoryRangeConfig {
                start: 0x60200000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
        };

        context
            .get_server()
            .create_machine(&default_config, &MachineRuntimeConfig::default())
            .await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_machine_create_already_created(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let mut default_config = context.get_server().get_default_config().await?;
        default_config.rom = RomConfig {
            bootargs: default_config.rom.bootargs,
            image_filename: String::from("/opt/cartesi/share/images/rom.bin"),
        };
        default_config.ram = RamConfig {
            length: 1 << 20,
            image_filename: String::new(),
        };
        default_config.uarch = UarchConfig {
            processor: Some(interfaces::UarchProcessorConfig {
                x: Some(vec![0; 32]),
                pc: Some(0x70000000),
                cycle: Some(0),
            }),
            ram: Some(interfaces::UarchRAMConfig {
                length: Some(77128),
                image_filename: Some(String::from("/opt/cartesi/share/images/uarch-ram.bin")),
            }),
        };
        default_config.htif = interfaces::HTIFConfig {
            console_getchar: Some(false),
            yield_manual: Some(true),
            yield_automatic: Some(false),
            fromhost: Some(0),
            tohost: Some(0),
        };
        default_config.rollup = RollupConfig {
            input_metadata: Some(MemoryRangeConfig {
                start: 0x60400000,
                length: 4096,
                image_filename: "".to_string(),
                shared: false,
            }),
            notice_hashes: Some(MemoryRangeConfig {
                start: 0x60800000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            rx_buffer: Some(MemoryRangeConfig {
                start: 0x60000000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            voucher_hashes: Some(MemoryRangeConfig {
                start: 0x60600000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            tx_buffer: Some(MemoryRangeConfig {
                start: 0x60200000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
        };
        let ret = context
            .get_server()
            .create_machine(&default_config, &MachineRuntimeConfig::default())
            .await;
        match ret {
            Ok(_) => panic!("Creating existing machine should fail"),
            Err(err) => assert_eq!(
                err.to_string(),
                "Jsonrpc cartesi machine error: Could not create new machine: ErrorObject { code: InvalidRequest, message: \"machine exists\", data: None }"
            ),
        }
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_run(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let run_response = context.get_server().run(1000).await?;
        assert_eq!(run_response, "reached_target_mcycle");

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_store(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context
            .get_server()
            .store(&format!("/tmp/cartesi_{}", generate_random_name()))
            .await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_store_nomachine(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let ret = context.get_server().store("/tmp/cartesi_store").await;
        assert!(ret.is_err());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_destroy(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context.get_server().destroy().await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_fork(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let address = context.get_server().fork().await?;
        let uri = format!("http://{}", address);
        JsonRpcCartesiMachineClient::new(uri).await.unwrap();
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_step(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let log = context
            .get_server()
            .step(
                &AccessLogType {
                    annotations: true,
                    proofs: true,
                },
                false,
            )
            .await?;
        //println!("Acquired log for step: {:?} ", log);
        assert!(log.accesses.len() > 0);
        assert!(log.accesses[0].r#type == AccessType::Read);
        assert!(log.brackets.len() > 0);
        assert!(log.log_type.proofs == true && log.log_type.annotations == true);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_shutdown(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context.get_server().shutdown().await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_double_shutdown(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context.get_server().shutdown().await?;
        let ret = context.get_server().shutdown().await;
        assert!(ret.is_err());
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_memory(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().read_memory(0x1000, 16).await?;
        assert_eq!(
            ret,
            vec![151, 2, 0, 0, 147, 130, 162, 4, 115, 144, 82, 48, 65, 101, 189, 101]
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_write_memory(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context
            .get_server()
            .write_memory(
                0x8000000F,
                STANDARD.encode([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            )
            .await?;
        let ret = context.get_server().read_memory(0x8000000F, 12).await?;
        assert_eq!(ret, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_word(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().read_word(0x100).await?;
        assert_eq!(ret, 0);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_root_hash(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().get_root_hash().await?;
        assert_eq!(ret, INITIAL_ROOT_HASH);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_root_hash_after_step(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().get_root_hash().await?;
        assert_eq!(ret, INITIAL_ROOT_HASH);
        let _log = context
            .get_server()
            .step(
                &AccessLogType {
                    annotations: true,
                    proofs: true,
                },
                false,
            )
            .await?;
        let ret = context.get_server().get_root_hash().await?;
        assert_eq!(ret, SECOND_STEP_HASH);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_proof(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let proof = context.get_server().get_proof(0x0, 10).await?;
        assert_eq!(proof.log2_target_size, 10);
        let mut target_hash_string = proof.target_hash.clone();
        if target_hash_string.ends_with('\n') {
            target_hash_string.pop();
        }
        assert_eq!(
            STANDARD.decode(target_hash_string).unwrap(),
            [
                223, 29, 35, 135, 156, 152, 209, 63, 32, 122, 132, 124, 77, 235, 34, 199, 91, 205,
                122, 232, 228, 246, 6, 38, 50, 162, 247, 189, 99, 218, 125, 54
            ]
        );
        assert_eq!(proof.sibling_hashes.len(), 54);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_replace_memory_range(
        context_with_machine_with_flash_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_with_flash_future.await;
        std::fs::write("/tmp/input.raw", b"test data 1234567890")?;
        std::process::Command::new("truncate")
            .args(&["-s", "62914560", "/tmp/input.raw"])
            .output()
            .expect("Unable to create flash image file");

        let memory_range_config = MemoryRangeConfig {
            start: 0x80000000000000,
            image_filename: "/tmp/input.raw".to_string(),
            length: 0x3c00000,
            shared: true,
        };
        context
            .get_server()
            .replace_memory_range(interfaces::MemoryRangeConfig::from(&memory_range_config))
            .await?;
        let ret = context
            .get_server()
            .read_memory(0x80000000000000, 12)
            .await?;
        assert_eq!(
            ret,
            vec![116, 101, 115, 116, 32, 100, 97, 116, 97, 32, 49, 50]
        );
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_x_address(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let x_address = context.get_server().get_x_address(2).await?;
        assert_eq!(x_address, 0x10);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_write_x(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let x_value = context.get_server().read_x(2).await?;
        assert_eq!(x_value, 0x0);
        context.get_server().write_x(2, 0x1234).await?;
        let x_value = context.get_server().read_x(2).await?;
        assert_eq!(x_value, 0x1234);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_reset_i_flags_y(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context.get_server().reset_iflags_y().await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_csr_address(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let address = context
            .get_server()
            .get_csr_address("pc".to_string())
            .await?;
        println!("Got address: {}", address);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_read_write_csr(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let x_value = context
            .get_server()
            .read_csr("sscratch".to_string())
            .await?;
        assert_eq!(x_value, 0x0);
        context
            .get_server()
            .write_csr("sscratch".to_string(), 0x12345)
            .await?;
        let x_value = context
            .get_server()
            .read_csr("sscratch".to_string())
            .await?;
        assert_eq!(x_value, 0x12345);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_initial_config(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let initial_config = context.get_server().get_initial_config().await?;
        println!("Acquired initial config {:?}", initial_config);
        assert_eq!(initial_config.processor.pc, 4096);
        assert_eq!(initial_config.processor.mvendorid, 7161130726739634464);
        assert_eq!(initial_config.processor.marchid, 0xf);
        assert_eq!(initial_config.ram.length, 1048576);
        assert_eq!(
            initial_config.rom.image_filename,
            "/opt/cartesi/share/images/rom.bin"
        );
        assert_eq!(initial_config.flash_drives.len(), 0);
        assert_eq!(initial_config.htif.fromhost, Some(0));
        assert_eq!(initial_config.htif.tohost, Some(0));
        assert_eq!(initial_config.clint.mtimecmp, Some(0));
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_verify_merkle_tree(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().verify_merkle_tree().await?;
        assert!(ret);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_verify_dirty_page_maps(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let ret = context.get_server().verify_dirty_page_maps().await?;
        assert!(ret);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_dump_pmas(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        context.get_server().dump_pmas().await?;
        std::thread::sleep(std::time::Duration::from_secs(3));
        std::process::Command::new("rm")
            .args(&[
                "0000000000000000--0000000000001000.bin",
                "0000000000001000--000000000000f000.bin",
                "0000000002000000--00000000000c0000.bin",
                "0000000040008000--0000000000001000.bin",
                "0000000080000000--0000000000100000.bin",
            ])
            .status()
            .expect("Failed to cleanup dump pmas test");
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_verify_access_log(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let log = context
            .get_server()
            .step(
                &AccessLogType {
                    annotations: true,
                    proofs: true,
                },
                false,
            )
            .await?;
        context
            .get_server()
            .verify_access_log(&log, &MachineRuntimeConfig::default(), false)
            .await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_verify_state_transition(
        context_with_machine_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_with_machine_future.await;
        let root_hash_before = context.get_server().get_root_hash().await?;
        let log = context
            .get_server()
            .step(
                &AccessLogType {
                    annotations: true,
                    proofs: true,
                },
                false,
            )
            .await?;
        let root_hash_after = context.get_server().get_root_hash().await?;
        context
            .get_server()
            .verify_state_transition(
                root_hash_before.to_vec(),
                &log,
                root_hash_after.to_vec(),
                false,
                &MachineRuntimeConfig::default(),
            )
            .await?;
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_sequential_requests(
        context_future: impl Future<Output = Context>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = context_future.await;
        let jsonrpc_machine = context.get_server();

        let mut default_config = jsonrpc_machine.get_default_config().await?;
        println!(
            "I have got default jsonrpc cartesi machine config: {:#?}",
            default_config
        );

        let x_addr = jsonrpc_machine.get_x_address(3).await?;
        println!("I got x address of register 3: {}", x_addr);

        let semantic_version = jsonrpc_machine.get_version().await?;
        println!("I got dhd  address of reg index 3: {:#?}", semantic_version);

        default_config.rom = RomConfig {
            bootargs: default_config.rom.bootargs,
            image_filename: String::from("/opt/cartesi/share/images/rom.bin"),
        };
        default_config.ram = RamConfig {
            length: 1 << 20,
            image_filename: String::new(),
        };

        default_config.uarch = UarchConfig {
            processor: Some(interfaces::UarchProcessorConfig {
                x: Some(vec![0; 32]),
                pc: Some(0x70000000),
                cycle: Some(0),
            }),
            ram: Some(interfaces::UarchRAMConfig {
                length: Some(77128),
                image_filename: Some(String::from("/opt/cartesi/share/images/uarch-ram.bin")),
            }),
        };

        default_config.rollup = RollupConfig {
            input_metadata: Some(MemoryRangeConfig {
                start: 0x60400000,
                length: 4096,
                image_filename: "".to_string(),
                shared: false,
            }),
            notice_hashes: Some(MemoryRangeConfig {
                start: 0x60800000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            rx_buffer: Some(MemoryRangeConfig {
                start: 0x60000000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            voucher_hashes: Some(MemoryRangeConfig {
                start: 0x60600000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
            tx_buffer: Some(MemoryRangeConfig {
                start: 0x60200000,
                length: 2 << 20,
                image_filename: "".to_string(),
                shared: false,
            }),
        };

        jsonrpc_machine
            .create_machine(&default_config, &MachineRuntimeConfig::default())
            .await?;

        let csr_addr = jsonrpc_machine.read_csr("mcycle".to_string()).await?;
        println!("I got csr address of mcycle reg: {}", csr_addr);

        let hash = jsonrpc_machine.get_root_hash().await?;
        println!("Root hash step 0 {:?}", hash);

        let access_log = jsonrpc_machine
            .step(
                &AccessLogType {
                    annotations: true,
                    proofs: true,
                },
                true,
            )
            .await?;
        println!(
            "Step performed, number of accesses: {} ",
            access_log.accesses.len()
        );

        let hash = jsonrpc_machine.get_root_hash().await?;
        println!("Root hash step 1 {:?}", hash);

        let run_info = jsonrpc_machine.run(100).await?;
        println!("Run info: {}", run_info);

        jsonrpc_machine.destroy().await?;
        println!("Machine destroyed");

        jsonrpc_machine.shutdown().await?;
        println!("Server shut down");

        Ok(())
    }
}
