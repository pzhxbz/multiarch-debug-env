mod args;
use args::{pasrse_args,modify_qemu_args,visit_dirs};
use std::{collections::HashMap, env, fs::{self, File, create_dir}, io::{Read, Write}, net::TcpStream};
use goblin::elf::Elf;
use std::{path::Path, process::exit,io::copy};
use nix::unistd::{chdir, sleep};
use subprocess::{Popen, PopenConfig, Redirection};
use ssh2::*;

macro_rules! err_exit {
    ($X:expr) => {
        print!("{}",$X);
        exit(0);
    };
    ($X1:expr,$X2:expr) => {
        print!($X1,$X2);
        exit(0);
    };
}
pub fn elf_arch_table()->HashMap<u16,String>{
    // todo : add more details for choosing rootfs
    let mut res:HashMap<u16,String> = HashMap::new();
    res.insert(40, String::from("armhf_le.zip"));
    res.insert(8, String::from("mips32r6.zip"));
    return res;
}

fn parse_elf(file_path:String)->String{
    let path = Path::new(file_path.as_str());
    let buffer = fs::read(path)
                        .expect("failed open binary file");
    let header = Elf::parse_header(&buffer)
                        .map_err(|_| "cannot parse ELF file").unwrap();
    return elf_arch_table()[&header.e_machine].clone();
}

fn extract_zip(zip_path:&str,out_dir:&str){
    let fname = std::path::Path::new(zip_path);
    let file = fs::File::open(&fname).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath =match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let outpath = Path::new(out_dir).join(outpath);
        if (&*file.name()).ends_with('/') {
            // println!("File {} extracted to \"{}\"", i, outpath.display());
            fs::create_dir_all(&outpath).unwrap();
        } else {
            // println!(
            //     "File {} extracted to \"{}\" ({} bytes)",
            //     i,
            //     outpath.display(),
            //     file.size()
            // );
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = fs::File::create(&outpath).unwrap();
            copy(&mut file, &mut outfile).unwrap();
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
            }
        }
    }
}

struct Prog{
    processs:Popen,
    stdin:Option<File> ,
    stdout:Option<File>
}
impl Prog {
    pub fn create_process(prog_argv:&[&str]) -> Prog{
        let mut res = Prog{
            processs: Popen::create(prog_argv, PopenConfig {
            stdout: Redirection::Pipe,
            stdin:Redirection::Pipe,
            ..Default::default()}).expect("unable to start program"),
            stdin:None,
            stdout:None
            };
        res.stdin = res.processs.stdin.take();
        res.stdout = res.processs.stdout.take();
        return  res;
    }
    pub fn read(self:&mut Prog)->String {
        // let (out,err) = self.processs.communicate(None).unwrap();
        // self.processs.stdout.as_ref().unwrap().flush().unwrap();
        let mut recv = [0; 1];
        self.stdout.as_ref().unwrap().read(&mut recv).expect("unable read process");
        // f.read(&mut recv);
        return String::from_utf8(recv.to_vec()).unwrap();
    }
    pub fn write(self:&mut Prog,input:String){
        // self.processs.communicate(Some(&input)).expect("unable to send data");
        self.stdin.as_ref().unwrap().write(input.as_bytes()).expect("unable write process");
        // self.processs.stdin.as_ref().unwrap().flush().unwrap();
    }
    #[allow(dead_code)]
    pub fn readall(self:&mut Prog)->String{
        let mut res = String::new();
        let mut t = self.read();
        while t.len() != 1 {
            res += &t;
            t = self.read();
        }
        return res;
    }
    pub fn send_line(self:&mut Prog,input:&str){
        self.write(input.to_string());
        self.write(String::from("\n"));
    }
    pub fn recv_until(self:&mut Prog,input:&str)->bool{
        let mut compare_flag = 0;
        let mut time_out = false;
        loop {
            let r = self.read();
            print!("{}",r);
            if r.len() == 0{
                if time_out{
                    return false; 
                }
                sleep(3); // time out
                time_out = true;
                continue;
            }
            time_out = false;
            if r.as_bytes()[0] == input.as_bytes()[compare_flag]{
                compare_flag += 1;
                if compare_flag == input.len(){
                    return true;
                }
            }
            else{
                compare_flag = 0;
            }
        }
    }
}

struct Connect{
    s:Session
}

impl Connect {
    pub fn create_connect(port:u32) -> Connect {
        let tcp = TcpStream::connect(format!("127.0.0.1:{}",port)).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();
        sess.userauth_password("root", "root").unwrap();
        assert!(sess.authenticated());
        return Connect{
            s:sess
        };
    }
    fn check_dir(self:&mut Connect,path:&str){
        
        let mut channel = self.s.channel_session().unwrap();
        channel.exec(&format!("if [ ! -d \"{}\" ]; then echo 1; else echo 0; fi\n",path)).unwrap();
        let mut s = String::new();
        channel.read_to_string(&mut s).unwrap();
        s = s.replace("\n", "").replace(" ", "");
        channel.wait_close().unwrap();
        let test = channel.exit_status().unwrap();
        // println!("check {} with {} exit with {}",path,s,test);
        if str::parse::<u32>(&s).unwrap() != 0 {
            self.check_dir(Path::new(path).parent().unwrap().to_str().unwrap());
            let mut channel = self.s.channel_session().unwrap();
            // println!("mkdir {}",path);
            channel.exec(&format!("mkdir {}",path)).unwrap();
            channel.read_to_string(&mut s).unwrap();
            channel.wait_close().unwrap();
        }
    }
    fn send_file(self:&mut Connect,path:&str){
        print!("sending {}  ...",path);
        let fname = Path::new(path);
        let mut file = File::open(&fname).unwrap();
        let metadata = fs::metadata(&path).expect("unable to read metadata");
        let mut buffer = vec![0; metadata.len() as usize];
        file.read(&mut buffer).unwrap();
        let remote_path = Path::new("/root/").join(path);
        self.check_dir(remote_path.parent().unwrap().to_str().unwrap());
        let mut remote_file = self.s.scp_send(&remote_path,
        0o777, metadata.len(), None).unwrap();
        remote_file.write(&buffer).unwrap();
        // Close the channel and wait for the whole content to be tranferred
        remote_file.send_eof().unwrap();
        remote_file.wait_eof().unwrap();
        remote_file.close().unwrap();
        remote_file.wait_close().unwrap();
        println!("finish");
    }
    pub fn upload(self:&mut Connect,path:&str){
        // Write the file
        let t = Path::new(path);
        if Path::new(path).is_dir(){
            let files = visit_dirs(&t);
            for f in files{
                self.send_file(&f);
            }
        }
        else{
            self.send_file(path);
        }
    }
    pub fn run_cmd(self:&mut Connect,cmd:&str){
        let mut channel = self.s.channel_session().unwrap();
        channel.exec(cmd).unwrap();
        let mut s = String::new();
        channel.read_to_string(&mut s).unwrap();
        print!("{}", s);
        channel.wait_close().unwrap();
        // print!(" exit code {}", channel.exit_status().unwrap());
    }
}

fn main() {
    let mut args = pasrse_args();
    // get elf arch
    if args.rootfs.len() == 0{
        args.rootfs = parse_elf(args.prog_name.clone());
    }
    // check debug env rootfs
    if !Path::new(&args.binary_path).exists(){
        err_exit!("Can't find root fs env");
    }
    let rootfs_path = args.binary_path.clone() + &args.rootfs;
    println!("use rootfs {}",args.rootfs);
    if !Path::new(&rootfs_path).exists(){
        err_exit!("Can't find {} fs env",rootfs_path);
    }
    // parpre workdir
    if !Path::new(&args.work_dir).exists(){
        create_dir(&args.work_dir).expect("Can't create work dir");
    }
    let arch_work_dir = args.work_dir.clone() + &args.rootfs;
    if !Path::new(&arch_work_dir).exists(){
        create_dir(&arch_work_dir).expect("Can't create arch work dir");
    }
    // print!("{} {}",rootfs_path,arch_work_dir);
    println!("extracted root fs files ...");
    extract_zip(&rootfs_path,&arch_work_dir);
    println!("parpre qemu args for launch ...");
    let launch_file = modify_qemu_args(&args,&arch_work_dir);
    let cur_dir  = env::current_dir().unwrap();
    chdir(Path::new(&arch_work_dir)).expect("can't chdir to work dir");
    // execvp(&CString::new("/bin/bash").unwrap(),&[&CString::new("/bin/bash").unwrap(),&CString::new(launch_file).unwrap()]).expect("launch fail");
    let mut p = Prog::create_process(&["/bin/bash",&launch_file]);
    // let mut p = Prog::create_process(&["/bin/sh"]);
    println!("launch qemu...");
    // login 
    p.recv_until("Welcome");
    p.send_line("");
    if !p.recv_until("login:"){
        err_exit!("can't login");
    }
    p.send_line("root");
    p.recv_until("Password:");
    p.send_line("root");
    if !p.recv_until("#"){
        err_exit!("login failed");
    }
    println!("login success");
    chdir(&cur_dir).unwrap();
    let mut c = Connect::create_connect(args.ssh_port);
    c.upload(&args.prog_name);
    for f in args.input_file.iter(){
        c.upload(&f);
    }
    let stdin = std::io::stdin();
    loop {
        let mut buf = String::new();
        stdin.read_line(&mut buf).unwrap();
        c.run_cmd(&buf);
    }
}
