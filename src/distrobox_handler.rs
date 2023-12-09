use crate::utils::get_command_output;
pub struct DBox {
    pub name: String,
    pub distro: String,
    pub image_url: String,
    pub container_id: String,
    pub status: String,
}

pub struct ColsIndexes {
    pub name: usize,
    pub image: usize,
    pub id: usize,
    pub status: usize,
}

pub fn get_all_distroboxes() -> Vec<DBox> {
    let mut my_boxes: Vec<DBox> = vec![];

    let cmd = "distrobox";
    let output = get_command_output(String::from(cmd), Some((&["list", "--no-color"])));
    //println!("output: {:?}", output);

    let headings = output
        .split("\n")
        .next()
        .unwrap()
        .split("|")
        .map(|h| h.trim())
        .collect::<Vec<&str>>();
    //println!("headings: {:?}", headings);

    let mut heading_indexes = ColsIndexes {
        name: 1,
        image: 3,
        id: 0,
        status: 2,
    };

    for (idx, heading) in headings.iter().enumerate() {
        match heading.as_ref() {
            "NAME" => heading_indexes.name = idx,
            "IMAGE" => heading_indexes.image = idx,
            "ID" => heading_indexes.id = idx,
            "STATUS" => heading_indexes.status = idx,
            _ => (),
        }
    }

    for (idx, line) in output.split("\n").enumerate() {
        if line == "" || idx == 0 {
            continue;
        }

        let box_line = line.split("|").map(|l| l.trim()).collect::<Vec<&str>>();
        if box_line.len() > 3 {
            my_boxes.push(DBox {
                name: String::from(box_line[heading_indexes.name]),
                distro: try_parse_distro_name_from_url(box_line[heading_indexes.image]),
                image_url: String::from(box_line[heading_indexes.image]),
                container_id: String::from(box_line[heading_indexes.id]),
                status: String::from(box_line[heading_indexes.status]),
            });
        }

        //println!("line: {:?}", line);
    }

    my_boxes
}

pub fn try_parse_distro_name_from_url(url: &str) -> String {
    let  distros = [
        "ubuntu",
        "debian",
        "centos",
        "oracle",
        "fedora",
        "arch",
        "alma",
        "slackware",
        "gentoo",
        "kali",
        "alpine",
        "clearlinux",
        "void",
        "amazon",
        "rocky",
        "redhat",
        "opensuse",
        "mageia",
    ];

    let mut distro_name = "zunknown";

    let last_part_of_url = url.split("/").last().unwrap_or("zunknown");

    for d in distros {
        if last_part_of_url.contains(d) {
            distro_name = d;
            break;
        }
    }

    if distro_name != "zunknown" {
        return distro_name.to_string();
    }

    for d in distros {
        if url.contains(d) {
            distro_name = d;
            break;
        }
    }

    distro_name.to_string()

}