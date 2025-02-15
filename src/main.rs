use config::File;
use config::{Config, ConfigError};
use discord_presence::{models::ActivityType, models::EventData, Client, Event};
use std::fs;
use std::io::prelude::*;
use std::process::exit;
use std::{env, thread, time};
use text_io::read;

fn help() {
    println!(
        "Usage:\n
    config\n
      Create the configuration file.\n
     start\n
       Start the jellyfin-rpc server."
    );
}

fn write_config(content: &[u8]) {
    let mut file = fs::File::create("config.json").unwrap();
    file.write_all(content).unwrap();
}

fn read_config(fp: &str) -> Result<Config, ConfigError> {
    let settings = Config::builder()
        .add_source(File::with_name(fp))
        .build()
        .unwrap_or_else(|_| panic!("Failed to read settings file: {:?}, please run the config command to set up the configuration file.", fp));
    Ok(settings)
}

async fn get_metadata(
    url: &String,
    endpoint: &str,
    key: &String,
) -> Result<serde_json::Value, reqwest::Error> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/{}", url, endpoint))
        .header("Authorization", format!("MediaBrowser Token={}", key))
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?;
    res.json::<serde_json::Value>().await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<String>>();

    match args.len() {
        1 => {
            println!("Type help to see a list of available commands.");
        }
        2 => match args[1].as_str() {
            "config" => {
                println!("Please enter your jellyfin API key.");
                let jellyfin_api_key: String = read!();
                println!("Please enter your discord client ID.");
                let discord_client_id: String = read!();
                println!("Please enter name of the user whose activity you wish to be displayed. ");
                let jellyfin_username: String = read!();
                println!("Please enter the url of the jellyfin server.");
                let jellyfin_url: String = read!();
                println!("Writing to config file...");
                let full_json = format!(
                    r#"
                {{
                "jellyfin_api_key": "{jellyfin_api_key}",
                "discord_client_id": "{discord_client_id}",
                "jellyfin_username": "{jellyfin_username}",
                "jellyfin_url": "{jellyfin_url}"
                }}
                "#
                );
                write_config(full_json.as_bytes());
            }

            "start" => {
                println!("Starting the jellyfin-rpc server.");

                println!("Getting configuration...");

                let config = read_config("config.json").unwrap();

                let config_api_key = config.get::<String>("jellyfin_api_key").unwrap();
                let mut config_discord_client_id = config
                    .get::<String>("discord_client_id")
                    .unwrap()
                    .parse::<u64>();
                let config_jellyfin_username = config.get::<String>("jellyfin_username").unwrap();
                let config_jellyfin_url = config.get::<String>("jellyfin_url").unwrap();
                let client = reqwest::Client::new();
                let session_endpoint = "Sessions?activeWithinSeconds=1";
                if config_jellyfin_url.is_empty() {
                    println!("Please enter your jellyfin API key!");
                    exit(1);
                }
                if (!config_discord_client_id.is_ok()) {
                    println!("No discord client id provided, falling back to default...");
                }
                if (config_jellyfin_username.is_empty()) {
                    println!("Please enter your Jellyfin username!");
                    exit(1);
                }


                println!("Testing connectivity to the server..");
                let res = client
                    .get(format!("{}/{}", config_jellyfin_url, session_endpoint))
                    .header(
                        "Authorization",
                        format!("MediaBrowser Token={config_api_key}"),
                    )
                    .send()
                    .await.expect("Something went wrong, did you enter your Jellyfin URL correctly?");

                let status_code = res.status();
                match status_code {
                    reqwest::StatusCode::OK => {
                        println!("Successfully connected to server.");
                    }
                    reqwest::StatusCode::UNAUTHORIZED => {
                        println!("We've been unauthorized, are you sure your API key is right?");
                        exit(1);
                    }
                    _ => {
                        println!("Something went wrong... {}", status_code);
                        exit(1);
                    }
                }
                println!("Connecting to discord RPC..");
                let mut drcp = Client::new(config_discord_client_id.unwrap_or(1338422523838206014));

                let _ = drcp.on_ready(|ctx| {
                    let EventData::Ready(data) = ctx.event else {
                        unreachable!();
                    };
                });
                drcp.start();

                drcp.block_until_event(Event::Ready)?;

                assert!(Client::is_ready());
                let mut previous_media_name = String::new();
                let mut sleep_duration = tokio::time::Duration::from_secs(5);
                loop {
                    let metadata =
                        get_metadata(&config_jellyfin_url, session_endpoint, &config_api_key)
                            .await?;
                    match metadata.get(0) {
                        None => continue,
                        Some(_media_name) => {
                            println!("User is playing media!");
                        }
                    }
                    let user_name = metadata[0]["UserName"].as_str().unwrap();
                    assert_ne!(user_name, "");
                    if user_name != config_jellyfin_username {
                        continue;
                    }
                    let now_playing_item = &metadata[0]["NowPlayingItem"];
                    let current_media_type = now_playing_item["MediaType"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let current_media_name =
                        now_playing_item["Name"].as_str().unwrap_or("").to_string();
                    let current_album_artist = now_playing_item["AlbumArtist"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let current_album =
                        now_playing_item["Album"].as_str().unwrap_or("").to_string();
                    let current_item_id = now_playing_item["Id"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let mut item_image_url;
                    let mut is_album_artist = false;
                    println!("Current media type: {}", current_media_type);

                    if current_media_name == previous_media_name {
                        sleep_duration = tokio::time::Duration::from_secs(10);
                    } else {
                        previous_media_name = current_media_name.to_string();
                        let a = match current_media_type.as_str() {
                            "Audio" => {
                                is_album_artist = true;
                                ActivityType::Listening
                            }
                            "Video" => ActivityType::Watching,
                            _ => ActivityType::Playing,
                        };
                        let resp = client
                            .get(format!(
                                "{}/Items/{}/Images/Primary",
                                config_jellyfin_url, current_item_id
                            ))
                            .header("Authorization", format!("MediaBrowser Token={config_api_key}"))
                            .send()
                            .await?;
                        match resp.status() {
                            reqwest::StatusCode::OK => {
                                item_image_url = resp.url().to_string();
                            }
                            _ => {
                                println!("{}", resp.status().as_str());
                                item_image_url = "https://play-lh.googleusercontent.com/FAlWhVMAjAzI6Nxc7bf4KPgjbwA3GT9j2bzeAMnRpdWim_2SXnS9i4zhwasKWIC8PV4".to_string()
                            }, /* I have no clue how to do this better */
                        }
                        match is_album_artist {
                            true => {
                                drcp.set_activity(|act| {
                                    act._type(a)
                                        .state(format!(
                                            "by {} on {}",
                                            current_album_artist, current_album
                                        ))
                                        .details(current_media_name)
                                        .assets(|a| a
                                            .large_image(item_image_url))
                                })
                                .expect("Panic!");
                            }
                            false => {
                                drcp.set_activity(|act| act._type(a).state(current_media_name))
                                    .ok();
                            }
                        }
                        let image_status_code =
                            sleep_duration = tokio::time::Duration::from_secs(5);
                        // Reset to normal interval
                    }

                    tokio::time::sleep(sleep_duration).await;
                }
            }

            "help" => {
                help();
            }
            _ => {
                println!(
                    "Unknown command: {}, type 'help' to see a list of available commands.",
                    args[1].as_str()
                );
            }
        },
        _ => {
            help();
        }
    }

    Ok(())
}
