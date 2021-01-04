use std::collections::{HashMap, HashSet};

struct EmitterStat {
    name: String,
    typ: String,
    desc: String,
}

pub struct Emitter {
    typedefs: HashMap<String, EmitterStat>,
    printed: HashSet<String>,
    out: String,
}

impl Emitter {
    pub fn new() -> Emitter {
        Emitter {
            typedefs: HashMap::new(),
            printed: HashSet::new(),
            out: String::new(),
        }
    }

    pub fn define(&mut self, statname: &str, stattype: &str, statdesc: &str) {
        self.typedefs.insert(statname.to_string(), EmitterStat {
            name: statname.to_string(),
            typ: stattype.to_string(),
            desc: statdesc.to_string(),
        });
    }

    fn emit_header(&mut self, stat_name: &str) {
        if self.printed.contains(stat_name) {
            return;
        }

        let es = self.typedefs.get(stat_name).unwrap();

        self.out += &format!("# TYPE {} {}\n", es.name, es.typ);
        self.out += &format!("# HELP {} {}\n", es.name, es.desc);

        self.printed.insert(stat_name.to_string());
    }

    pub fn emit_i64(&mut self, stat_name: &str, host: &str, job: &str,
        val: i64)
    {
        self.emit_header(stat_name);

        let es = self.typedefs.get(stat_name).unwrap();
        self.out += &format!("{}{{host=\"{}\",name=\"{}\"}}\t{}\n",
            es.name, host, job, val);
    }

    pub fn emit_i32(&mut self, stat_name: &str, host: &str, job: &str,
        val: i32)
    {
        self.emit_i64(stat_name, host, job, val as i64);
    }

    pub fn out(&self) -> &str {
        self.out.as_str()
    }
}
