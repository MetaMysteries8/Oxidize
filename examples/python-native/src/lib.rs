use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyInt;

#[pyfunction]
fn count_primes(limit: &Bound<'_, PyAny>) -> PyResult<usize> {
    if !limit.is_exact_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("limit must be an integer"));
    }
    if limit.lt(0)? {
        return Err(PyValueError::new_err("limit must be non-negative"));
    }
    if limit.gt(1_000_000)? {
        return Err(PyValueError::new_err("limit too large"));
    }
    let limit: usize = limit.extract()?;
    let mut composite = vec![false; limit];
    let mut count = 0;
    for number in 2..limit {
        if !composite[number] {
            count += 1;
            if number <= (limit - 1) / number {
                for multiple in (number * number..limit).step_by(number) {
                    composite[multiple] = true;
                }
            }
        }
    }
    Ok(count)
}

#[pymodule]
fn _oxidize_5cf14915940d(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(count_primes, m)?)?;
    Ok(())
}
