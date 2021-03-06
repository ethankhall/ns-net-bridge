# ns-net-bridge
Connecting docker containers and host networking

## Why does this exist?

Docker is great, but network namespaces are hard. By default docker doesn't like to share it's namespace info with the system. That makes it hard to use an `ip netns` command to inspect it.

Even if you were able to get a process into the network namespace, that's not really that useful because now it can't listen on the host socket to connect to it.

This tool makes it so that you can have one tool that connects a port inside a docker container to the host network.

## How does this work?

There are two thread pools. The `host` and the `namespace`. Any incoming request is processed by the `host` threadpool. When a connection is made, the tool creates a new connection on the `namespace` threadpool to the designated target. The tool will then shuffel data transparently container was running on the hosts IP address.

## How do I use it?

Assuming both `ns-net-bridge` and the `connect` script are on the `PATH` on the TARGET machine.

```
export PORT=8080
ssh -L $PORT:127.0.0.1:$PORT $TARGET
docker ps # find the docker container id
connect $DOCKER_ID $PORT
```

And you will now be able to access something on port 8080 from your local machine and have it routed to the docker container.
