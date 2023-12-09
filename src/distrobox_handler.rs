pub struct DBox {
   pub  name: String,
   pub  distro: String,
   pub  image_url: String,
   pub  container_id: String,
   pub  status: String,
}

pub fn get_all_distroboxes() -> Vec<DBox> {
    let mut my_boxes: Vec<DBox> = vec![];

    let db = DBox {
        name: String::from("debian-11"),
        distro: String::from("Debian 11"),
        image_url: String::from("https://hub.docker.com/_/debian"),
        container_id: String::from("debian-11"),
        status: String::from("Ready"),
    };
    let db_arch = DBox {
        name: String::from("Arch"),
        distro: String::from("Arch Linux"),
        image_url: String::from("https://hub.docker.com/_/debian"),
        container_id: String::from("abc123"),
        status: String::from("Running"),
    };

    my_boxes.push(db);
    my_boxes.push(db_arch);

    my_boxes
}