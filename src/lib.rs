use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::sync::PoisonError;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
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
	last_id:Arc<AtomicUsize>,
	working:Arc<AtomicBool>
}
impl ThreadQueue {
	pub fn new() -> ThreadQueue {
		let (ss,sr) = mpsc::channel();
		let last_id = Arc::new(AtomicUsize::new(0));
		let working = Arc::new(AtomicBool::new(true));

		let sender_map = Arc::new(RwLock::new(HashMap::<usize,Sender<Notify>>::new()));

		{
			let sender_map = Arc::clone(&sender_map);
			let working = Arc::clone(&working);

			let mut current_id = 0usize;

			std::thread::spawn(move || {
				while working.load(Ordering::Acquire) {
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
							}
						},
						Notify::Quit if sender_map.read().unwrap().is_empty()=> {
							break;
						},
						Notify::Quit => {
							panic!("There are still threads waiting to be processed.");
						}
						_ => (),
					}
				}
			});
		}

		ThreadQueue {
			sender_map:sender_map,
			sender:ss,
			last_id:last_id,
			working:working
		}
	}

	pub fn submit<F,T>(&mut self,f:F) ->
		Result<JoinHandle<T>,PoisonError<RwLockWriteGuard<'_,HashMap<usize,Sender<Notify>>>>>
		where F: FnOnce() -> T, F: Send + 'static, T: Send + 'static {

		let (cs,cr) = mpsc::channel();

		let last_id = self.last_id.fetch_add(1, Ordering::SeqCst);

		{
			self.sender_map.write()?.insert(last_id, cs.clone());
		}

		let ss = self.sender.clone();

		let id = last_id;

		let r = std::thread::spawn(move || {
			ss.send(Notify::Started(id)).unwrap();
			cr.recv().unwrap();

			let r = f();

			ss.send(Notify::Terminated(id)).unwrap();
			r
		});

		Ok(r)
	}
}
impl Drop for ThreadQueue {
	fn drop(&mut self) {
		self.working.store(false, Ordering::Release);
		let _ = self.sender.send(Notify::Quit);
	}
}