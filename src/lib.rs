use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::sync::PoisonError;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
pub enum Notify {
	Started(usize),
	Go,
	Terminated(usize),
	Quit,
}
pub struct ThreadQueue {
	sender_map:Arc<RwLock<HashMap<usize,Sender<Notify>>>>,
	sender:Sender<Notify>,
	last_id:Arc<RwLock<usize>>,
	working:Arc<AtomicBool>,
	busy_count:Arc<AtomicUsize>,
	worker_handler:Option<JoinHandle<()>>
}
impl ThreadQueue {
	pub fn new() -> ThreadQueue {
		let (ss,sr) = mpsc::channel();

		let last_id = Arc::new(RwLock::new(0));
		let working = Arc::new(AtomicBool::new(true));
		let busy_count = Arc::new(AtomicUsize::new(0));

		let sender_map = Arc::new(RwLock::new(HashMap::<usize,Sender<Notify>>::new()));

		let h = {
			let sender_map = Arc::clone(&sender_map);
			let working = Arc::clone(&working);
			let busy_count = Arc::clone(&busy_count);

			let mut current_id = 0usize;

			let (ws,wr) = mpsc::channel();

			let h = std::thread::spawn(move || {
				ws.send(()).unwrap();

				loop {
					match sr.recv().unwrap() {
						Notify::Started(id) => {
							if id == current_id {
								sender_map.read()
									.unwrap().get(&id)
									.unwrap().send(Notify::Go).unwrap();
							}
						},
						Notify::Terminated(id) => {
							if id == current_id {
								match sender_map.write() {
									Ok(mut map) => {
										map.remove(&id);
										let id = id + 1;
										current_id = id;
										match map.get(&id) {
											Some(ref sender) => {
												sender.send(Notify::Go).unwrap();
											},
											None => ()
										}
									},
									Err(ref e) => {
										panic!("{:?}", e);
									}
								};

								busy_count.fetch_sub(1, Ordering::Release);

								if !working.load(Ordering::Acquire) && busy_count.load(Ordering::Acquire) == 0 {
									break;
								}
							}
						},
						Notify::Quit if busy_count.load(Ordering::Acquire) > 0 => {

						},
						Notify::Quit if sender_map.read().unwrap().is_empty() => {
							break;
						},
						Notify::Quit => {
							panic!("There are still threads waiting to be processed.");
						}
						_ => (),
					}
				}
			});

			wr.recv().unwrap();

			h
		};

		ThreadQueue {
			sender_map:sender_map,
			sender:ss,
			last_id:last_id,
			working:working,
			busy_count:Arc::clone(&busy_count),
			worker_handler:Some(h)
		}
	}

	pub fn submit<F,T>(&mut self,f:F) ->
		Result<JoinHandle<T>,PoisonError<RwLockWriteGuard<'_,HashMap<usize,Sender<Notify>>>>>
		where F: FnOnce() -> T, F: Send + 'static, T: Send + 'static {

		let (cs,cr) = mpsc::channel();

		match self.last_id.write().unwrap() {
			mut last_id => {
				{
					self.sender_map.write()?.insert(*last_id, cs.clone());
				}

				let ss = self.sender.clone();

				let id = *last_id;

				let (on_start_sender,on_start_receiver) = mpsc::channel();

				let busy_count = Arc::clone(&self.busy_count);

				let r = std::thread::spawn(move || {
					ss.send(Notify::Started(id)).unwrap();
					busy_count.fetch_add(1, Ordering::Release);
					on_start_sender.send(()).unwrap();
					cr.recv().unwrap();

					let r = f();

					ss.send(Notify::Terminated(id)).unwrap();
					r
				});

				*last_id += 1;

				on_start_receiver.recv().unwrap();

				Ok(r)
			}
		}
	}
}
impl Drop for ThreadQueue {
	fn drop(&mut self) {
		let r = self.sender.send(Notify::Quit);
		self.working.store(false, Ordering::Release);
		self.worker_handler.take().map(|h| {
			h.join().unwrap()
		}).unwrap();
		r.unwrap();
	}
}