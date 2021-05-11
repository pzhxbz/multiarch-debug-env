mod args;
use args::{pasrse_args,modify_qemu_args};
use std::{collections::HashMap, fs::{self, create_dir}};
use goblin::elf::Elf;
use std::{path::Path, process::exit,io::copy};
use nix::unistd::{chdir, execvp};
use std::ffi::CString;

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
    chdir(Path::new(&arch_work_dir)).expect("can't chdir to work dir");
    execvp(&CString::new("/bin/bash").unwrap(),&[&CString::new("/bin/bash").unwrap(),&CString::new(launch_file).unwrap()]).expect("launch fail");
}
