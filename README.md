# queuingtask
Rustで順番に異なるスレッドを実行するためのライブラリ
## 使い方
    let mut thread_queue = ThreadQueue::new();
    thread_queue.submit(move || {
      print!("aaaaaa");
      1
    });
  ※スレッドはsubmitに渡した順番で順次実行されます。
  前のスレッドの実行が終了するまで次のスレッドはブロックされます。

### 戻り値を受け取る

    let h = thread_queue.submit(move || {
      print!("aaaaaa");
      1
    });
    /// スレッドの終了を待機
    let r = h.join().unwrap();
