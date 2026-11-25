use pyo3::prelude::*;

#[pyfunction]
fn run_json(py: Python<'_>, input: String) -> String {
    py.detach(|| entityres::run_json(&input))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_json, m)?)
}
