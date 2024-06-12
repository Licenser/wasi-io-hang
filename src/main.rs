use anyhow::Result;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt, DuplexStream};
use wasmtime::{Config, Engine, Linker, Module, Store, TypedFunc};
use wasmtime_wasi::{
    pipe::{AsyncReadStream, AsyncWriteStream},
    preview1::{self, WasiP1Ctx},
    AsyncStdinStream, AsyncStdoutStream, WasiCtxBuilder,
};

struct WasiRuntime {
    store: Store<WasiP1Ctx>,
    engine: Engine,
    linker: Linker<WasiP1Ctx>,
    stdin: DuplexStream,
    stdout: DuplexStream,
}
impl WasiRuntime {
    fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config)?;
        let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
        preview1::add_to_linker_async(&mut linker, |t| t)?;

        let (stdin, stdin_wasi) = tokio::io::duplex(1024);
        let (stdout, stdout_wasi) = tokio::io::duplex(1024);

        let stdin_stream = AsyncReadStream::new(stdin_wasi);
        let stdout_stream = AsyncWriteStream::new(1024, stdout_wasi);
        let wasi_ctx = WasiCtxBuilder::new()
            .stdin(AsyncStdinStream::new(stdin_stream))
            .stdout(AsyncStdoutStream::new(stdout_stream))
            .inherit_stderr()
            .build_p1();

        let store = Store::new(&engine, wasi_ctx);
        Ok(WasiRuntime {
            store,
            engine,
            linker,
            stdin,
            stdout,
        })
    }

    async fn get_instance(&mut self, archive: &[u8]) -> Result<TypedFunc<(), ()>> {
        let module = Module::from_binary(&self.engine, archive)?;
        Ok(self
            .linker
            .module_async(&mut self.store, "", &module)
            .await?
            .get_default(&mut self.store, "")?
            .typed::<(), ()>(&self.store)?)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let archive = tokio::fs::read("target/wasm32-wasi/debug/adder.wasm")
        .await
        .expect("Failed to read wasi file, please run 'cargo build -p adder --target wasm32-wasi'");

    let mut runtime = WasiRuntime::new()?;
    let func = runtime.get_instance(&archive).await?;

    let executor = tokio::task::spawn(async move {
        dbg!();
        func.call_async(&mut runtime.store, ()).await?;
        dbg!();
        anyhow::Ok(())
    });

    let mut stdin = runtime.stdin;
    let mut stdout = runtime.stdout;

    // prepare the writer side

    stdin.write_all(b"1\n").await?;
    stdin.write_all(b"2\n").await?;
    stdin.write_all(b"3\n").await?;
    stdin.write_all(b"4\n").await?;
    stdin.write_all(b"5\n").await?;
    stdin.flush().await?;
    drop(stdin);

    // let buf = BufReader::new(runtime.stdout);
    // let mut lines = buf.lines();
    let mut buf = [0u8; 2];
    dbg!("reading ");
    stdout.read_exact(&mut buf).await.expect("nodata");
    assert_eq!(b"3\n", &buf);
    println!("first read succeded");
    stdout.read_exact(&mut buf).await.expect("nodata");
    assert_eq!(b"7\n", &buf);
    println!("second read succeded");

    assert!(stdout.read_exact(&mut buf).await.is_err());

    executor.await.expect("runtime failed")?;

    Ok(())
}
