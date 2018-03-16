use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::collections::HashMap;
use std::num::Wrapping;

#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
pub enum Notify {
	Started(u32),
	Go,
	Terminated(u32),
}
pub struct ThreadQueue {
	sender_map:Arc<Mutex<HashMap<u32,Sender<Notify>>>>,
	sender:Option<Sender<Notify>>,
	current_id:u32,
	last_id:u32,
}
impl ThreadQueue {
	pub fn new() -> ThreadQueue {
		ThreadQueue {
			sender_map:Arc::new(Mutex::new(HashMap::new())),
			sender:None,
			current_id:0,
			last_id:0,
		}
	}

	pub fn submit<F,T>(&mut self,f:F) ->
		Result<JoinHandle<T>,PoisonError<MutexGuard<HashMap<u32,Sender<Notify>>>>>
		where F: FnOnce() -> T, F: Send + 'static, T: Send + 'static {

		if self.sender_map.lock()?.len() == 0 {
			let (ss,sr) = mpsc::channel();
			let current_id = Arc::new(Mutex::new(self.current_id));
			let sender_map = self.sender_map.clone();

			std::thread::spawn(move || {
				while sender_map.lock().unwrap().len() == 0 {
					std::thread::sleep(std::time::Duration::from_millis(1));
				}
				while sender_map.lock().unwrap().len() > 0 {
					match sr.recv().unwrap() {
						Notify::Started(id) => {
							match current_id.lock() {
								Ok(ref mut current_id) if id == **current_id => {
									sender_map.lock()
										.unwrap().get(&id)
										.unwrap().send(Notify::Go).unwrap();
								},
								Ok(_) => (),
								Err(ref e) => {
									panic!(format!("{:?}",e));
								}
							}
						},
						Notify::Terminated(id) => {
							match current_id.lock() {
								Ok(ref mut current_id) if id == **current_id => {
									match sender_map.lock() {
										Ok(mut map) => {
											map.remove(&id);
											let id = Wrapping(id) + Wrapping(1);
											let id = id.0;
											**current_id = id;
											match map.get(&id) {
												Some(ref sender) => {
													sender.send(Notify::Go).unwrap();
												},
												None => ()
											}
										},
										Err(ref e) => {
											panic!(format!("{:?}",e));
										}
									};
								},
								Ok(_) => (),
								Err(ref e) => {
									panic!(format!("{:?}",e));
								}
							}
						},
						_ => (),
					}
				}
			});
			self.sender = Some(ss)
		}

		let (cs,cr) = mpsc::channel();

		self.sender_map.lock()?.insert(self.last_id, cs.clone());

		let ss = match self.sender {
			Some(ref ss) => ss.clone(),
			None => panic!("Sender is not initialized."),
		};

		let id = self.last_id;
		let last_id = Wrapping(self.last_id) + Wrapping(1);

		self.last_id = last_id.0;

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