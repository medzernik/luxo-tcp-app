use interface::ClientData;

mod binary_message;
mod client_commands;
mod interface;
mod stream;

#[derive(Debug)]
/// Mode to use when connecting.
pub enum ConnectionMode {
    TCP,
    UNIX,
}

fn main() -> Result<(), String> {
    // We take the user's arguments.
    // For the client, there must be 2 arguments, type of connection and connection port/unix socket name.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err(
            "incorrect number of arguments provided. use `server TYPE PORT` where TYPE = TCP|UNIX"
                .to_string(),
        );
    }

    let server_type = args[1].as_str();
    let port = args[2].as_str();

    // We decide which mode to use depending on the user's arguments
    match server_type {
        "TCP" => {
            // Creating the dynamic object so we can use the same function for both modes
            let (stream, mut client_state) = ClientData::new(ConnectionMode::TCP, port.to_string());

            client_state.await_input(stream);
        }
        "UNIX" => {
            let (stream, mut client_state) =
                ClientData::new(ConnectionMode::UNIX, port.to_string());
            client_state.await_input(stream);
        }

        _ => Err("argument of TYPE is not set to either 'TCP' or 'UNIX'".to_string()),
    }
}
