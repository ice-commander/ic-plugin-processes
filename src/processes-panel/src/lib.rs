use ic_plugin_api::{IcColumn, IcHost, IcTable};
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

pub const PLUGIN_ID: &str = "processes";
pub const PLUGIN_ICON: &str = include_str!("../assets/processes.svg");
pub const BACK_ICON: &str = include_str!("../assets/back.svg");
pub const KILL_ICON: &str = include_str!("../assets/kill.svg");

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRow {
    pub name: String,
    pub pid: u32,
    pub memory: u64,
    pub cpu: f32,
}

pub fn collect() -> Vec<ProcessRow> {
    let mut sys = System::new_all();
    sys.refresh_processes();
    let mut rows: Vec<ProcessRow> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcessRow {
            name: p.name().to_string(),
            pid: pid.as_u32(),
            memory: p.memory(),
            cpu: p.cpu_usage(),
        })
        .collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    rows
}

pub fn format_memory(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_cpu(cpu: f32) -> String {
    format!("{:.1}%", cpu)
}

struct Snapshot {
    columns: Vec<IcColumn>,
    cells: Vec<*const c_char>,
    owned: Vec<CString>,
    titles: Vec<CString>,
}

impl Snapshot {
    fn new() -> Self {
        Self {
            columns: Vec::new(),
            cells: Vec::new(),
            owned: Vec::new(),
            titles: Vec::new(),
        }
    }
}

thread_local! {
    static SNAPSHOT: RefCell<Snapshot> = RefCell::new(Snapshot::new());
}

const HEADINGS: [(&str, &str, i32); 4] = [
    ("name", "Name", 260),
    ("pid", "PID", 90),
    ("memory", "Memory", 110),
    ("cpu", "CPU", 90),
];

extern "C" fn process_rows(_user_data: *mut c_void) -> IcTable {
    let rows = collect();
    SNAPSHOT.with(|s| {
        let mut slot = s.borrow_mut();
        *slot = Snapshot::new();

        for (key, title, _) in HEADINGS.iter() {
            slot.titles.push(CString::new(*key).unwrap_or_default());
            slot.titles.push(CString::new(*title).unwrap_or_default());
        }
        for row in &rows {
            for text in [
                row.name.clone(),
                row.pid.to_string(),
                format_memory(row.memory),
                format_cpu(row.cpu),
            ] {
                slot.owned.push(CString::new(text).unwrap_or_default());
            }
        }

        let columns: Vec<IcColumn> = HEADINGS
            .iter()
            .enumerate()
            .map(|(i, (_, _, width))| IcColumn {
                key: slot.titles[i * 2].as_ptr(),
                title: slot.titles[i * 2 + 1].as_ptr(),
                width: *width,
            })
            .collect();
        slot.columns = columns;
        slot.cells = slot.owned.iter().map(|c| c.as_ptr()).collect();

        IcTable {
            columns: slot.columns.as_ptr(),
            column_count: HEADINGS.len() as u32,
            cells: slot.cells.as_ptr(),
            row_count: rows.len() as u32,
            key_column: 1,
        }
    })
}

pub fn kill_process(pid: u32) -> Result<(), String> {
    let mut sys = System::new_all();
    sys.refresh_processes();
    match sys.processes().values().find(|p| p.pid().as_u32() == pid) {
        Some(p) if p.kill() => Ok(()),
        Some(_) => Err(format!("could not end process {}", pid)),
        None => Err(format!("process {} is not running", pid)),
    }
}

extern "C" fn on_back_clicked(user_data: *mut c_void, _parent: *mut c_void) {
    let host = user_data as *const IcHost;
    if host.is_null() {
        return;
    }
    unsafe {
        ((*host).close_panel_source)();
    }
}

extern "C" fn on_kill_clicked(user_data: *mut c_void, _parent: *mut c_void) {
    let host = user_data as *const IcHost;
    if host.is_null() {
        return;
    }
    let selection = unsafe { ((*host).selection)() };
    let pids: Vec<u32> = selection
        .as_slice()
        .iter()
        .filter_map(|i| i.key_string())
        .filter_map(|k| k.parse::<u32>().ok())
        .collect();
    for pid in pids {
        if let Err(text) = kill_process(pid) {
            if let Ok(msg) = CString::new(text) {
                unsafe { ((*host).log_warn)(msg.as_ptr()) };
            }
        }
    }
    if let Ok(id) = CString::new(PLUGIN_ID) {
        unsafe { ((*host).open_panel_source)(id.as_ptr()) };
    }
}

extern "C" fn on_button_clicked(user_data: *mut c_void, _parent_window: *mut c_void) {
    let host = user_data as *const IcHost;
    if host.is_null() {
        return;
    }
    let Ok(id) = CString::new(PLUGIN_ID) else {
        return;
    };
    unsafe {
        ((*host).open_panel_source)(id.as_ptr());
    }
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../version.rs"));

ic_plugin_api::declare_about!(
    "ic-processes-panel",
    "Processes",
    plugins_version!(),
    "Lists running processes in a panel"
);

#[cfg_attr(feature = "export-abi", no_mangle)]
pub extern "C" fn ic_plugin_init(host: *const IcHost, _kind: *const c_char) -> c_int {
    match ic_plugin_api::check_host(
        host,
        ic_plugin_api::IC_ABI_VERSION,
        std::mem::size_of::<IcHost>() as u32,
    ) {
        ic_plugin_api::HostCheck::Ok => {}
        ic_plugin_api::HostCheck::WrongMagic => return ic_plugin_api::IC_ERR_HOST_UNKNOWN,
        _ => return ic_plugin_api::IC_ERR_HOST_TOO_OLD,
    }
    let (Ok(id), Ok(title), Ok(svg)) = (
        CString::new(PLUGIN_ID),
        CString::new("Processes"),
        CString::new(PLUGIN_ICON),
    ) else {
        return ic_plugin_api::IC_ERR_INIT_FAILED;
    };
    let registered = unsafe {
        ((*host).register_panel_source)(
            id.as_ptr(),
            title.as_ptr(),
            svg.as_ptr(),
            process_rows,
            std::ptr::null_mut(),
        )
    };
    if registered != ic_plugin_api::IC_OK {
        return registered;
    }
    let (Ok(back_id), Ok(back_tip), Ok(back_svg)) = (
        CString::new("processes.back"),
        CString::new("Back"),
        CString::new(BACK_ICON),
    ) else {
        return ic_plugin_api::IC_ERR_INIT_FAILED;
    };
    let (Ok(kill_id), Ok(kill_tip), Ok(kill_svg)) = (
        CString::new("processes.kill"),
        CString::new("End process"),
        CString::new(KILL_ICON),
    ) else {
        return ic_plugin_api::IC_ERR_INIT_FAILED;
    };
    unsafe {
        ((*host).register_panel_action)(
            id.as_ptr(),
            back_id.as_ptr(),
            back_svg.as_ptr(),
            back_tip.as_ptr(),
            ic_plugin_api::IC_ENABLE_ALWAYS,
            on_back_clicked,
            host as *mut c_void,
        );
        ((*host).register_panel_action)(
            id.as_ptr(),
            kill_id.as_ptr(),
            kill_svg.as_ptr(),
            kill_tip.as_ptr(),
            ic_plugin_api::IC_ENABLE_ON_FILE,
            on_kill_clicked,
            host as *mut c_void,
        );
    }
    unsafe {
        ((*host).add_toolbar_button)(
            id.as_ptr(),
            svg.as_ptr(),
            title.as_ptr(),
            ic_plugin_api::IC_SIDE_RIGHT,
            20,
            ic_plugin_api::IC_ENABLE_ALWAYS,
            on_button_clicked,
            host as *mut c_void,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_is_running_something() {
        let rows = collect();
        assert!(!rows.is_empty(), "a live system always has processes");
        assert!(rows.iter().all(|r| !r.name.is_empty()));
    }

    #[test]
    fn rows_come_back_sorted_by_name() {
        let rows = collect();
        let mut sorted = rows.clone();
        sorted.sort_by_key(|r| r.name.to_lowercase());
        assert_eq!(rows, sorted);
    }

    #[test]
    fn memory_is_written_the_way_a_person_reads_it() {
        assert_eq!(format_memory(512), "512 B");
        assert_eq!(format_memory(2048), "2 KB");
        assert_eq!(format_memory(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_memory(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn cpu_keeps_one_decimal() {
        assert_eq!(format_cpu(0.0), "0.0%");
        assert_eq!(format_cpu(12.34), "12.3%");
    }

    #[test]
    fn the_table_describes_four_columns_and_matches_its_rows() {
        let table = process_rows(std::ptr::null_mut());
        assert_eq!(table.column_count, 4);
        assert_eq!(table.columns_slice().len(), 4);
        assert_eq!(table.columns_slice()[0].key_string(), "name");
        assert_eq!(table.columns_slice()[3].title_string(), "CPU");
        assert!(table.row_count > 0);
        let first_name = table.cell(0, 0).unwrap_or_default();
        assert!(!first_name.is_empty());
        let pid = table.cell(0, 1).unwrap_or_default();
        assert!(pid.parse::<u32>().is_ok(), "the second column is a pid");
    }

    #[test]
    fn ending_a_process_that_is_not_there_says_so() {
        let err = kill_process(u32::MAX).unwrap_err();
        assert!(err.contains("not running"), "got: {err}");
    }
}
