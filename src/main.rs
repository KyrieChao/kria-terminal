use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "kria-terminal", version, about = "结构化命令执行器")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    // 执行一个程序并返回结构化结果
    Run {
        // 工作目录，默认当前目录
        #[arg(long)]
        cwd: Option<String>,
        // 要执行的程序
        program: String,
        // 传给该程序的参数
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    // 先只打印解析结果，验证对不对
    println!("{:#?}", cli);
}
