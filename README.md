# queuingtask
Rustで順番に異なるスレッドを実行するためのライブラリ
## 使い方
    extern crate queuingtask;
     
    let mut thread_queue = ThreadQueue::new();
    thread_queue.submit(move || {
      print!("aaaaaa");
      1
    });
  ※スレッドはsubmitに渡した順番で順次実行されます。
  前のスレッドの実行が終了するまで次のスレッドはブロックされます。
#### Cargo.toml
    [package]
    name = "hoge"
    version = "0.1.0"
    authors = ["yourname"]

    [dependencies.queuingtask]
    git = "https://github.com/j6k1/queuingtask.git"
### 戻り値を受け取る

    let h = thread_queue.submit(move || {
      print!("aaaaaa");
      1
    });
    /// スレッドの終了を待機
    let r = h.join().unwrap();
