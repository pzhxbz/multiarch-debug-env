use std::{env, fs::{File, read_dir}, io:: BufReader, path::Path, process::exit};
use std::collections::HashMap;
use indoc::indoc;
use std::io::prelude::*;

fn print_help(){
    print!(indoc! {"mutiarch debug {}
    USAGE:
        multiarch-debug [FLAGS] [OPTIONS] <INPUT_FILE> [args]
    
    FLAGS:
        -h                           Prints help information
        -socat                       use socat to bind program's io to socket
        -chroot                      use chroor to run program
    
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
        "},env!("CARGO_PKG_VERSION"));
    exit(0);
}

pub struct Args{
    pub    gdb_port:u32,
    pub    ssh_port:u32,
    pub    prog_port:u32,
    pub    forward_port:Vec<(u32,u32)>,
    pub    qemu_arg:String,
    pub    prog_arg:Vec<String>,
    pub    prog_name:String,
    pub    env:HashMap<String,String>,
    pub    input_file:Vec<String>,
    pub    work_dir:String,
    pub    rootfs:String,
    pub    binary_path:String,
    pub    no_socat:bool,
    pub    chroot:bool
}
#[allow(dead_code)]
impl Args {
    fn print_fotmat(self:&Args){
        print!(indoc! {"
            port        :{}
            qemu_arg    :{}
            prog_arg    :{:?}
            prog_name   :{}
            env         :{:?}
            input_file  :{:?}
        "},self.gdb_port,self.qemu_arg,self.prog_arg,self.prog_name
           ,self.env,self.input_file);
    }
}
pub fn pasrse_args()->Args{
    let args: Vec<String> = env::args().collect();
    // print!("{}",args[0]);
    if args.len() == 1{
        print_help();
    }
    let mut res =  Args{
        forward_port:Vec::new(),
        gdb_port:1234,
        ssh_port:2222,
        qemu_arg:String::new(),
        prog_arg:Vec::new(),
        env:HashMap::new(),
        input_file:Vec::new(),
        prog_name:String::new(),
        work_dir:String::from("/tmp/multiarch_debug.env/"),
        rootfs:String::new(),
        prog_port:23333,
        no_socat:true,
        // add multiarch-rootfs-env for args[0]
        binary_path:Path::new(&args[0]).parent().unwrap().join("multiarch-rootfs-env/").to_str().unwrap().to_string(),
        chroot:true
    };
    let mut i=1;
    let mut find_prog_name = false;
    while i < args.len(){
        let a = &args[i];
        if find_prog_name{
            res.prog_arg.push(a.to_string());

        }
        else if a == "-g"{
            i += 1;
            res.gdb_port = args[i].parse()
                        .expect("failed to parse gdb port\n");
        }
        else if a == "-p"{
            i += 1;
            res.prog_port = args[i].parse()
                        .expect("failed to parse prog port\n");
        }
        else if a == "-a"{
            i += 1;
            res.input_file = args[i].split(",").map(|s| s.to_string()).collect();
        }
        else if a == "-h"{
            print_help();
        }
        else if a == "-e"{
            i += 1;
            res.qemu_arg = args[i].to_string();
        }
        else if a == "-s"{
            i += 1;
            res.ssh_port = args[i].parse()
                                .expect("failed to parse ssh port\n");
        }
        else if a == "-socat"{
            res.no_socat = false;
        }
        else if a == "-chroot"{
            res.chroot = true;
        }
        else if a == "-ep"{
            i += 1;
            let port1:u32 = args[i].parse()
                            .expect("failed to parse port1\n");
            i += 1;
            let port2:u32 = args[i].parse()
                            .expect("failed to parse port2\n");
            res.forward_port.push((port1,port2));

        }
        else if a == "-env"{
            i += 1;
            let t:Vec<&str> = args[i].split("=").collect();
            if t.len()<2{
                print!("failed parse env");
                exit(0);
            }
            res.env.insert(t[0].to_string(), t[1].to_string());
        }
        else if a == "-w"{
            i += 1;
            res.work_dir = args[i].to_string();
        }
        else if a == "-f"{
            i += 1;
            res.rootfs = args[i].to_string() + ".zip";
        }
        // prog name
        else{
            find_prog_name = true;
            res.prog_name = a.to_string();
        }
        i += 1;
    }
    // res.print_fotmat();
    return res;
} 

pub fn visit_dirs(dir: &Path) -> Vec<String> {
    let mut res:Vec<String> = Vec::new();
    if dir.is_dir() {
        for entry in read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                let mut t1 = visit_dirs(&path);
                res.append(&mut t1);
            }
            else{
                res.push(path.display().to_string());
            }
        }
    }
    return res;
}

fn read_qemu_args(launsh_file:&str) -> String{
    let file = File::open(launsh_file).expect("can't open launsh bash file");
    let mut contents = String::new();
    let mut buf_reader = BufReader::new(file);
    buf_reader.read_to_string(&mut contents).expect("can't read launsh bash file");
    let mut res = String::new();
    let mut net_flag = 0;
    const NET_USER: &str = "-net user";
    for i in contents.chars(){
        if net_flag == 0 && i != '-'{
            res.push(i);
        }
        else if (net_flag & 0xf) < NET_USER.len(){
            res.push(i);
            if i != NET_USER.to_string().as_bytes()[net_flag & 0xf] as char{
                net_flag = 0;
                continue;
            }
            net_flag += 1;
        }
        else{
            // 0x100 last char is blank ?
            // 0x1000 mark the blank between -net and hostfwd options
            // 0x10000 has add {} mark
            if (net_flag & 0x10000) != 0{
                res.push(i);
            }
            else if i == ' '{
                if (net_flag & 0x1000)!= 0{
                    if (net_flag & 0x10000) == 0{
                        res += "{MARK_HERE}";
                        net_flag |= 0x10000;
                    }
                    res.push(i);
                    continue;
                }
                net_flag |= 0x100;
            }
            else if (net_flag & 0x1000) == 0{
                net_flag |= 0x1000;
            }
        }
    }
    return res;
}

pub fn modify_qemu_args(args:&Args,work_dir:&str) -> String{
    let files = visit_dirs(Path::new(work_dir));
    let mut ori_sh_files = String::new();
    for f in files{
        if f.contains(".sh"){
            if !f.contains("debug-env"){
                ori_sh_files = f;
                break;
            }
        }
    }
    let qemu_args = read_qemu_args(&ori_sh_files);

    // print!("{}",qemu_args);
    let mut port_forward = format!(",hostfwd=tcp::{}-:{}",args.gdb_port,1234);
    port_forward += format!(",hostfwd=tcp::{}-:{}",args.ssh_port,22).as_str();
    if !args.no_socat{
        port_forward += format!(",hostfwd=tcp::{}-:{}",args.prog_port,23333).as_str();
    }
    for p in args.forward_port.iter(){
        port_forward += format!(",hostfwd=tcp::{}-:{}",p.0,p.1).as_str();
    }
    let mut l = qemu_args.len() -1;
    while qemu_args.as_bytes()[l] as char == ' ' || qemu_args.as_bytes()[l]  as char == '\n' {
        l-=1;
    }
    // qemu arg still buggy
    // println!("{}",&args.qemu_arg);
    let res = qemu_args[..(l+1)].replace("{MARK_HERE}",&port_forward) ;//+ &args.qemu_arg;
    // println!("{}",res); 
    let file_path = Path::new(work_dir).join("debug-env.sh").display().to_string();
    let mut f = File::create(&file_path).unwrap();
    f.write_all(res.as_bytes()).expect("");
    f.sync_all().expect("");
    return file_path;
}