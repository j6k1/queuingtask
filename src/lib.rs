use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;

#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
pub enum Notify {
	Started(u64),
	Go,
	Terminated(u64),
}
pub struct ThreadQueue {
	sender_map:Arc<Mutex<HashMap<u64,Sender<Notify>>>>,
	sender:Option<Sender<Notify>>,
	current_id:u64,
	last_id:u64,
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

	pub fn submit<F>(&mut self,f:F) ->
		Result<(),PoisonError<MutexGuard<HashMap<u64,Sender<Notify>>>>>
		where F: Fn(), Arc<F>: Send + 'static  {

		if self.sender_map.lock()?.len() == 0 {
			let (ss,sr) = mpsc::channel();
			let current_id = Arc::new(Mutex::new(self.current_id));
			let sender_map = self.sender_map.clone();

			std::thread::spawn(move || {
				while sender_map.lock().unwrap().len() > 0 {
					let id = sr.recv().unwrap();

					match id {
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
											let id = id + 1;
											**current_id = id;
												map.get(&id)
													.unwrap().send(Notify::Go).unwrap();
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

		let ss = match self.sender {
			Some(ref ss) => ss.clone(),
			None => panic!("Sender is not initialized."),
		};

		self.sender_map.lock()?.insert(self.current_id, cs.clone());

		let f = Arc::new(f);
		let id = self.last_id;

		self.last_id = self.last_id + 1;

		std::thread::spawn(move || {
			ss.send(Notify::Started(id)).unwrap();
			cr.recv().unwrap();

			f();

			ss.send(Notify::Terminated(id)).unwrap();
		});

		Ok(())
	}
}