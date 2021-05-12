# multiarch-debug-env

Debug binary use qemu-system

Automatic send file into qemu and launch binary through socat or directly

use `./build.sh` to build the project

usage:
```
    multiarch_debug [FLAGS] [OPTIONS] <INPUT_FILE> [args]
    
    FLAGS:
        -h                           Prints help information
        -socat                       use socat to bind program's io to socket
    
    OPTIONS:
        -a <FILE1,FILE2,DIR1>        Add files or dirs send to qemu
        -e 'ARGS'                    Sets the ext args for qemu
        -g <port>                    Sets the gdbserver's port,default 1234
        -s <port>                    Sets the ssh server's port,default 2222
        -p <port>                    Sets the socat bind program io port,default 23333
        -ep <port1> <port2>          append extra forward port
        -env LD_PRELOAD=./libc       add env to binary
        -h                           Prints this message or the help
        -w                           workdir,default is /tmp/multiarch_debug.env/
        -f                           rootfs name,default will parse from elf's e_machine
    
    ARGS:
        <INPUT_FILE>                 Sets the input file to debug
        <args>                       Sets the args to binary
```

