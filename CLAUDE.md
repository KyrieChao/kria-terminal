# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目现状

做一个属于自己的终端（Terminal）。当前进度：**自研 shell 内核的第一阶段已完成**——REPL、lexer、parser、builtin 四个模块可运行，外部命令已能通过 `std::process::Command` 执行（靠 `spawn` + `wait` 继承父进程的 stdio）。**PTY / VT 解析 / 渲染 / ToolRunner 尚未开始。**

**实现路线（用户 2026-11-18 确认）：先用现成库做出能跑的版本，后续是否自研核心（VT 解析 / PTY / 渲染 / ToolRunner）视情况再定。** 不要拿"必须纯手写"当硬约束，也不要在基础设施上卡住出不了成果；每个借用现成实现的模块，在代码里标注一下"将来可作为自研候选"即可。

语言不限，Rust 是主选（安全性），项目内可以多语言混用。`Cargo.toml` 的 `clap` / `serde` / `serde_json` 是为后续 ToolRunner 的结构化 I/O 预留的，**当前源码尚未使用**。

设计依据见项目外的笔记：`D:\Lan\文件\东方瑞通\笔记\工具使用说明\Kria\Kria-15_2026_11_06.md`（这个终端项目的核心设计文档）。

## 环境状态

| 组件 | 路径 / 状态 |
| :--- | :--- |
| rustc / cargo | `D:\Computer_Language\Rust\.cargo\bin\`，版本 **1.95.0** |
| python | `D:\Computer_Language\python\python.exe` |
| git | `D:\Computer_Language\mingw64\usr\bin\git.exe` |
| MSYS2 | `D:\Computer_Language\mingw64\`（含 mingw64 / ucrt64 / clang64 环境，自带 bash、gcc、make） |

**注意：Rust 不在 Git Bash 的 PATH 里**，在 bash 中直接 `cargo build` 会 command not found。用绝对路径调用，或先执行：

```bash
export PATH="/d/Computer_Language/Rust/.cargo/bin:$PATH"
```

Shell 是 Windows 上的 **Git Bash（MSYS2）**，不是 cmd/PowerShell：用 `/c/...` 风格路径，`where` 而不是 `which`。

**查工具、查路径、查版本，一律直接问用户**，不要自己用 `where` / `ls` / 遍历 PATH 去探测（用户全局偏好，且之前误判过一次）。

## 当前代码结构

自研 shell 内核是最顶端的一层（`src/main.rs` 的 `mod` 声明），数据流是 **读行 → lex → parse → dispatch**：

```
main.rs   REPL 循环 + Shell{ cwd, prev } 状态 + 命令分派
  ├─ lexer.rs    &str → Vec<Token>（Word / Pipe / RedirectOut / Append / RedirectIn）
  ├─ parser.rs   Vec<Token> → Pipeline{ Vec<Command{ program, args }> }
  └─ builtin.rs  pwd / cd（含 cd -、跨盘符）/ exit / clear / ls
```

几个理解代码时必须知道的设计点：

- **`Shell` 是可变状态，`ShellSnapshot` 是只读快照。** `dispatch` 拿 `&mut Shell`（cd 要改 cwd），其余执行路径应拿快照。`main.rs` 里的 `run_command` / `run_builtin` 目前是 `todo!()` 桩函数，代表着向"统一 stdin/stdout 重定向"演进的中间状态——真正在用的是 `builtin::dispatch` + `run_external`。
- **词法层已经能产出 `RedirectOut` / `Append` / `RedirectIn`，但 parser 会直接报"暂不支持重定向"**。加重定向支持时，改动落在 `parser.rs`（结构体要扩字段）和 `builtin::dispatch` 的输出路径两端，不是 lexer。
- **管道在词法/语法层已被解析成多个 `Command`，但 `main` 目前对 `>1` 个命令直接打印"暂不支持管道"。**
- **外部命令用 `spawn` + `wait`，而不是 `output()`**：输出直接透传到父终端，shell 本身的屏幕输出由 `print!` 直接写 stdout。这意味着当前版本还没有字节流截获，**VT 渲染和"读取屏幕网格"都要等 PTY 层建立后才能做**。
- `cd` 的路径处理（`normalize` / `resolve_cd` / `commit_cd`）刻意手写而非用 `canonicalize`：`normalize` 保留相对 `..`、正确处理 Windows 盘符与 UNC 前缀，且**不要求目标已存在**（因为 `cd -` 的目标可能被删）。改动这块务必跑 `tests/unit/builtin.rs` 里的 Windows 用例。

### 测试布局（局部，容易找错）

单元测试**不写在模块内部**，而是放在 `tests/unit/*.rs`，再由各模块用 `#[path = "../tests/unit/xxx.rs"] mod tests;` 挂回 `#[cfg(test)]`。所以 `src/lexer.rs` 末尾没有测试代码，测试在 `tests/unit/lexer.rs`。注意 `tests/clear.rs` 是**集成测试**（走 `CARGO_BIN_EXE_kria-terminal` 启动子进程），与 `tests/unit/` 平行，不参与单元测试模块挂载。

**已知基线问题：`tests/clear.rs` 当前失败。** 它把 `current_dir` 设为编译机的源码目录，`cargo test` 的工作目录与之相同，在 `spawn` 时直接 `NotFound`（不是断言失败）。这是环境问题不是逻辑问题，改动前先确认它在当前机器上是否仍失败，别误判成回归。

## 核心架构设计（务必遵循）

这个终端与普通终端模拟器的根本区别在于：**它有两个完全独立的执行通道**，而不是把所有输入都交给 shell 解析。

```
┌─────────────────────────────────────────────────┐
│                终端应用                           │
│                                                  │
│  ┌──────────────┐        ┌──────────────────┐   │
│  │ 交互式 PTY 通道 │        │ 程序化执行通道     │   │
│  │ (用户敲命令)   │        │ (AI 调工具)       │   │
│  │              │        │                  │   │
│  │ PTY 字节流 I/O │        │ std::process::   │   │
│  │ VT 解析渲染    │        │ Command          │   │
│  └──────────────┘        │ 结构化 Output     │   │
│         │                └──────────────────┘   │
│         ▼                        │               │
│   屏幕网格 / 用户                ▼               │
│                          { exit_code,            │
│                            stdout, stderr }      │
│                          喂给 AI API             │
└─────────────────────────────────────────────────┘
```

### 通道一：交互式 PTY

- 用户输入的普通命令走 PTY，与真实 shell 交互。
- 字节流需要自己做 **VT 序列解析**（ANSI 转义、光标移动、擦除、SGR 颜色等）并渲染到屏幕网格（screen grid）。
- 这层是给人看的，保留回显、颜色、交互式程序（vim、ssh、进度条）的行为。
- **当前实现还不是通道一**：没有 PTY，`read_line` 直接读 stdin，外部命令靠 `spawn` 继承父进程终端。PTY 层对接 **ConPTY**（Windows 10 1809+ 的伪控制台 API），第一版先用封装好的 crate 跑通，之后再考虑用 `windows-sys` 直接调 API 替换。

### 通道二：程序化执行（AI 工具）

- AI 的工具调用**绝不经过 shell 字符串拼接**，直接 `program + args + cwd`。
- 通过 `std::process::Command` 执行，显式指定 `current_dir`。
- 返回结构化的 `{ success, exit_code, stdout, stderr }`，**stdout 与 stderr 绝不合并**（`2>&1` 会毁掉 AI 对错误的理解）。
- 环境变量：`NO_COLOR=1`（去掉 ANSI 颜色码，避免污染 AI 上下文）、`LANG=C.UTF-8`（稳定输出编码）。

### `ToolRunner` 的职责（项目核心）

笔记中已有 `run_tool` / `ToolRunner` 的完整草稿，是**程序化执行通道的唯一下层入口**。要点：

1. **路径解析 `resolve_program`**——解决"找不到 `./ft.exe`"这类问题：
   - 绝对路径 → 直接用；
   - 含 `/` 或 `\` 的显式相对路径 → `cwd.join(p)`，存在即返回；
   - 裸名字 → 依次查 `cwd` → 配置的额外搜索目录 `search_paths` → 系统 `PATH`；
   - Windows 上裸名字自动补 `.exe`。
   - 可执行文件所在位置应作为**配置**（`search_paths`），而不是把 `./` 编码进命令字符串。
2. **显式工作目录**——`cwd` 参数必传或使用配置的 `default_cwd`，绝不依赖进程当前目录。
3. 后续还要加上：**超时**、**输出截断**（防 AI 上下文爆炸）、**并发限制**、**危险命令分级**（safe / dangerous / destructive，`rm -rf` 之类强制人工确认）。

### AI 工具调用协议

AI 返回结构化参数，而不是 shell 字符串：

```json
{ "program": "make", "args": [], "cwd": "/project" }
```

ToolRunner 执行后**原样结构化**回传：

```json
{ "success": false, "exit_code": 2, "stdout": "", "stderr": "src/main.c:12:5: error: 'foo' undeclared" }
```

AI 根据上一步的 `success`/`exit_code` 自己决定下一步——`&&` 短路语义由 AI 侧承担，不再存在于 shell 字符串里。

### 两种输入模式（UI 设计）

1. **普通模式**：输入直接进 PTY，给人用。
2. **AI 模式**：`?` 或 `Ctrl+K` 触发，输入自然语言/斜杠命令，终端把**最近的命令历史 + 结构化结果**打包成上下文发给 AI；AI 的工具调用走 ToolRunner 通道，结果既回给 AI 也显示在屏幕上。

AI 发起的命令在屏幕上回显一份，但用**不同颜色标注为"AI 执行"**，执行本身走结构化通道。

## 可借鉴的开源设计（只借鉴思路，不把它们当作依赖）

| 项目 | 值得移植的思路 |
| :--- | :--- |
| `cleat` | 在生成的 shell 中注入 **OSC 133** 语义标记，把 PTY 字节流切分为 `{ stdout, exit_code, completed }`，并保持 `cd`/`export` 跨命令的会话状态 |
| `vterm-rs` | **`screen_read` / `wait_until`**：让 AI 读取屏幕网格状态（可见 `vim`、进度条的真实渲染），而非只看 stdout；**原子批处理**减少往返 |
| `agentic-terminal` | **文件夹感知**：AI 工具调用默认在会话当前工作目录执行，`cd` 是粘性的；敏感命令分级 |
| `TerminAI` | 意图 → 计划 → 批准 → 执行 → 审计日志；能处理 `sudo`/`ssh`/`vim` 交互式会话 |

## 常用命令

项目根目录下（bash 里需先 `export PATH="/d/Computer_Language/Rust/.cargo/bin:$PATH"`）：

```bash
cargo build                       # 构建
cargo run                         # 运行（交互式 REPL，Ctrl+D / exit 退出）
cargo test                        # 全部测试
cargo test <test_name>            # 只跑名字匹配的子集（bin 测试目标）
cargo test --test clear           # 只跑集成测试 clear
cargo clippy                      # lint
cargo fmt                         # 格式化
```

> 这个 crate 是 **bin-only**（没有 `src/lib.rs`），`tests/unit/*.rs` 是被 `#[path]` 挂进 bin 测试目标的模块，所以单测过滤直接 `cargo test <子串>`；**不要用 `--lib`**，当前会报没有 lib 目标。

快速手动验证外部命令通道（stdin 喂命令）：

```bash
printf 'pwd\necho hi\n' | ./target/debug/kria-terminal.exe
```

## 命名规范（Kria 项目约定，笔记 Kria-16 确定）

| 代码元素 | 命名方式 | 示例 |
| :--- | :--- | :--- |
| 结构体 / 枚举 / 类 / Trait / 类型别名 / 枚举成员 / 泛型 | PascalCase | `UserInfo`、`NetworkState`、`Connected` |
| 函数 / 方法 / 变量 / 参数 / 字段 / 模块 | snake_case | `calculate_max`、`user_name` |
| 常量 | SCREAMING_SNAKE_CASE | `MAX_BUFFER_SIZE` |

缩写统一按单词处理：`HttpRequest`、`JsonParser`、`TcpServer`。

## 协作方式（重要）

用户要的**不是**现成代码的交付，而是**被引导着一步步把代码写出来**，并且能用自己的话说清每一步在做什么、为什么这么做。

- 每次只推进一小步（一个模块 / 一个函数 / 一个技术决策），讲清动机与替代方案，再让他动手或确认。
- 每步结束用提问验收：让他不看代码复述这一步。复述不出来就停下补，而不是继续往下堆。
- 技术权衡摆成 pros & cons 表格，让他自己拍板。
- 所有回复与代码注释使用**简体中文**（用户全局偏好）。
- 注释只在"为什么"不明显时写，不要写解释代码在做什么的水分注释。
- Rust 中 `unsafe`（如 Win32 API 调用）用一行中文注明约束原因。
