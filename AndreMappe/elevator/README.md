# Distributed Elevator System

This project implements a distributed elevator system written in Rust.
The system is designed to satisfy the specifications given in the Elevator Project in the course TTK4145 Real-time Programming at NTNU.

## Build

Compile the project with:

```bash
cargo build
``` 

## Run

There is two ways to run the program. If you want to run it on separate machines use

```bash
cargo run
```

on each of the separate machines. This will use the machines MAC-adress to create a unique ID for the elevator and it will connect to the elevator hardware/simulator on the `BASE_DRIVER_PORT` in the config, the default port is `15657`.

If you want to run the program on the same machine you need to specify a unique ID when starting the program with 

```bash
cargo run <id>
```
This will give the elevator the ID you specified and will connect to the elevator hardware/simulator on the `BASE_DRIVER_PORT + <id>`. So if you want to run three elevators on one machine you could run

```bash
cargo run 0
cargo run 1
cargo run 2
```
in three separate terminals. This would with the default `BASE_DRIVER_PORT` connect to port `15657`, `15658` and `15659`.

**Note for Linux users:**
The program may need execute permissions for the 'hall_request_assigner' file.
If you encounter a "Permission denied" error, run:

```bash
chmod +x hall_request_assigner
```

## Documentation

The documentation is written in the source code using Rust doc comments  (`///`). 

It can be generated with: 
```bash
cargo doc --no-deps --open
```

This will build the documentation and open it in your browser. 

If using Visual Studio Code it is recommended to install the rust-analyzer extension, as this will show the documentation while hovering over functions and types.