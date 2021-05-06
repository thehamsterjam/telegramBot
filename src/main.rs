use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{env, str};
use telegram_bot::*;

#[derive(Deserialize, Debug)]
struct SearchResult {
    id: i64,
}
#[derive(Deserialize, Debug)]
struct SearchResults {
    data: Vec<SearchResult>,
}

#[derive(Deserialize, Debug)]
struct SpotifyToken {
    access_token: String,
    token_type: String,
    scope: String,
    expires_in: i32,
}
#[derive(Deserialize, Debug)]
struct SpotifySearchResult {
    tracks: SpotifySearchResultTracks,
}
#[derive(Deserialize, Debug)]
struct SpotifySearchResultTracks {
    items: Vec<SpotifySearchResultItems>,
}
#[derive(Deserialize, Debug)]
struct SpotifySearchResultItems {
    uri: String,
}

#[derive(Serialize, Debug)]
struct SpotifyAddToPlaylilst {
    uris: Vec<String>,
    position: i32,
}

const DEEZER_SEARCH_URL: &str = "https://api.deezer.com/search";
const DEEZER_PLAYLIST_URL: &str = "https://api.deezer.com/playlist/8866431842/tracks";


const SPOTIFY_ACCESS_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const SPOTIFY_SEARCH_URL: &str = "https://api.spotify.com/v1/search";
const SPOTIFY_PLAYLIST_URL: &str = "https://api.spotify.com/v1/playlists/4BvNLwSbqsrwtHXZ1erfAz/tracks";

#[tokio::main]
async fn main() -> Result<(), Error> {

    let token: String = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not found");
    let deezer_token: String = env::var("DEEZER_TOKEN").expect("DEEZER_TOKEN not found") ;
    let _spotify_client_id: String = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID not found");
    let _spotify_secret: String = env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET not found");
    let spotify_refresh_token : String = env::var("SPOTIFY_REFRESH_TOKEN").expect("SPOTIFY_REFRESH_TOKEN not found");
    let spotify_basic_auth : String = env::var("SPOTIFY_BASIC_AUTH").expect("SPOTIFY_BASIC_AUTH not found");

    let api = Api::new(token);
    let http_client = reqwest::Client::new();

    // Fetch new updates via long poll method
    let mut stream = api.stream();
    while let Some(update) = stream.next().await {
        // If the received update contains a new message...
        let update = update?;
        if let UpdateKind::Message(message) = update.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                // Print received text message to stdout.
                println!("<{}>: {}", &message.from.first_name, data);

                if data.contains("https://son.gg/t/") {
                    let song: Vec<&str> = data.split("\n").collect();
                    println!("Found song reference {}", song[0]);
                    add_to_deezer_playlist(&http_client, &deezer_token, song[0]).await;
                    add_to_spotify_playlist(&http_client, &spotify_refresh_token, &spotify_basic_auth, song[0]).await;
                }
            }
        }
    }

    Ok(())
}

async fn add_to_deezer_playlist(deezer_client: &reqwest::Client, deezer_token: &String, song: &str) {
    println!("---DEEZER---");
    let search_resp = deezer_client
        .get(DEEZER_SEARCH_URL)
        .query(&[("access_token", &deezer_token), ("q", &&song.to_owned())])
        .send()
        .await;

    match search_resp {
        Ok(resp) => {
            let results_resp = resp.json::<SearchResults>().await;
            match results_resp {
                Ok(results) => {
                    println!("Song id = {}", results.data[0].id);

                    let response = deezer_client
                        .get(DEEZER_PLAYLIST_URL)
                        .query(&[
                            ("access_token", &deezer_token),
                            ("songs", &&results.data[0].id.to_string()),
                            ("request_method", &&"POST".to_owned()),
                        ])
                        .send()
                        .await;

                    if let Err(error) = response {
                        println!("Error adding song to playlist, {}", error);
                    } else {
                        println!("Successfully added song");
                    }
                }
                Err(err) => println!("Error deserialising search results {}", err),
            }
        }
        Err(err) => println!("Error fetching search results {}", err),
    }
}

async fn add_to_spotify_playlist(spotify_client: &reqwest::Client, spotify_refresh_token: &String, spotify_basic_auth: &String, song: &str) {
    println!("---SPOTIFY---");

    let mut params = HashMap::new();
    params.insert("grant_type", "refresh_token");
    params.insert("refresh_token", spotify_refresh_token);

    let access_token_resp = spotify_client
        .post(SPOTIFY_ACCESS_TOKEN_URL)
        .form(&params)
        .header("Authorization", spotify_basic_auth)
        .send()
        .await;

    match access_token_resp {
        Ok(resp) => {
            println!("Got spotify token");
            let access_token = resp.json::<SpotifyToken>().await;

            match access_token {
                Ok(tok) => {
                    let search_resp = spotify_client
                        .get(SPOTIFY_SEARCH_URL)
                        .query(&[("q", &song), ("type", &"track"), ("limit", &"1")])
                        .bearer_auth(tok.access_token.clone())
                        .send()
                        .await;

                    match search_resp {
                        Ok(resp) => {
                            let spotify_search = resp.json::<SpotifySearchResult>().await;

                            match spotify_search {
                                Ok(search_res) => {
                                    let add_playlist_resp = spotify_client
                                        .post(SPOTIFY_PLAYLIST_URL)
                                        .header("Content-Type", "application/json")
                                        .bearer_auth(tok.access_token)
                                        .json(&SpotifyAddToPlaylilst {
                                            uris: vec![search_res.tracks.items[0].uri.clone()],
                                            position: 0
                                        })
                                        .send()
                                        .await;

                                    match add_playlist_resp {
                                        Ok(resp) => {
                                            println!("Done adding song to playlist");
                                            println!("{}", resp.text().await.unwrap());
                                        }
                                        Err(err) => {
                                            println!("Error adding song to playlist {}", err)
                                        }
                                    }
                                }
                                Err(err) => {
                                    println!("Failed to unwrap search result {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            println!("Failed to get search result {}", err);
                        }
                    }
                }
                Err(err) => {
                    println!("Couldnt unwrap token {}", err);
                }
            }
        }
        Err(err) => {
            println!("Couldnt get access token {}", err);
        }
    }
}
