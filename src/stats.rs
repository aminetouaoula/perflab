pub fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        values.sort_by(|a, b| a.total_cmp(b));
        let mid = values.len() / 2;

        // if leng is even
        if values.len().is_multiple_of(2) {
            let result = (values[mid - 1] + values[mid]) / 2.0;
            Some(result)
        } else {
            let result = values[mid];
            Some(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn median_single_value() {
        let mut v = vec![7.0];
        assert_eq!(median(&mut v), Some(7.0));
    }

    #[test]
    fn median_with_duplicates() {
        let mut v = vec![5.0, 5.0, 1.0, 5.0];
        assert_eq!(median(&mut v), Some(5.0));
    }

    #[test]
    fn median_negative_values() {
        let mut v = vec![-3.0, 0.0, -1.0];
        assert_eq!(median(&mut v), Some(-1.0));
    }
    #[test]
    fn median_odd_count() {
        let mut v = vec![3.0, 1.0, 2.0];
        assert_eq!(median(&mut v), Some(2.0));
    }

    #[test]
    fn median_even_count() {
        let mut v = vec![4.0, 1.0, 3.0, 2.0];
        assert_eq!(median(&mut v), Some(2.5));
    }

    #[test]
    fn median_empty_is_none() {
        let mut v: Vec<f64> = vec![];
        assert_eq!(median(&mut v), None);
    }
}
