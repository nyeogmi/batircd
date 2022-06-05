mod cancel;
mod directory;
mod parse;
mod protocol;
mod room;
mod subscriptions;
mod sock;
mod user;
mod world;

use world::World;

#[tokio::main(flavor="multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let listener = TcpListener::bind("127.0.0.1:6667").await?;

    let mut world = World::new();
    world.main_loop().await?;
    Ok(())
}
