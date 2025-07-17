# RustMailer Docker Deployment Guide

## Build the Project
First, compile the project using the following command:

```bash
./build.sh
```
## Run the Container
There are two ways to start the container:

### Method 1: Specify environment variables one by one
```bash
sudo docker run -d  -p 15630:15630 -v /sourcecode/rustmailer_data:/data -e RUSTMAILER_ROOT_DIR=/data -u $(id -u youruser):$(id -g yourgroup) rustmailer:1.0.0
```

### Method 2: Use an environment file
```bash
docker run -d \
  -v /host/data:/data \
  --env-file env.list \
  rustmailer:1.0.0
```

###Access the Container Shell

To manually access the shell inside the running container, use:
```bash
sudo docker run --rm -it --entrypoint sh rustmailer:1.0.0
```
### 

## Default Ports
RustMailer uses the following default ports:
- REST API Port: `15630`
- Web UI Port: `15630`
- gRPC Ports: `16630`


