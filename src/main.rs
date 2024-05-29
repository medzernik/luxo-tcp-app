fn main() {
    println!(
        "
        To run the application, run as follows:
        Server requires type of connection, port and password.
        Client requires type of connection and a port.
        The port is the name of the unix pipe. Automatically created at '/tmp/PORT'.
        To run more clients, simply launch more terminals and launch multiple clients.
        I recommend spectating in the browser.

        Spectate on 127.0.0.1:PORT
        You have to refresh to get new data.

        Be advised, the client must run the same type as the server to connect.

            SERVER - TCP,
            cargo run --bin server TCP 8080 dota2
            
            CLIENT A - TCP,
            cargo run --bin client TCP 8080

            CLIENT B - TCP,
            cargo run --bin client TCP 8080

            CLIENT C - TCP,
            cargo run --bin client TCP 8080

            SERVER - UNIX,
            cargo run --bin server UNIX luxo_server_pipe dota2

            CLIENT A - UNIX,
            cargo run --bin client UNIX luxo_server_pipe
    "
    );
}
