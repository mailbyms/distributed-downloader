//! Download range distributor

/// Download distributor for dividing file ranges among multiple parts
pub struct DownloadDistributor;

impl DownloadDistributor {
    /// Calculate download intervals for multiple parts
    ///
    /// This function divides the range [left_point, right_point] into number_of_parts chunks
    /// and returns a vector of intervals [start, end] for each part.
    pub fn download_interval_list(left_point: u64, right_point: u64, number_of_parts: usize) -> Vec<[u64; 2]> {
        let length = right_point - left_point + 1;

        // If length is less than number_of_parts, we can't divide properly
        if length < number_of_parts as u64 {
            panic!("Number of parts ({}) is greater than file size ({})", number_of_parts, length);
        }

        let base_interval = length / number_of_parts as u64;
        let remainder = length % number_of_parts as u64;

        // Create size list with base intervals
        let mut size_list = vec![base_interval; number_of_parts];

        // Distribute remainder among the parts
        for i in 0..remainder as usize {
            size_list[i] += 1;
        }

        // Calculate cumulative sizes
        for i in 1..number_of_parts {
            size_list[i] += size_list[i - 1];
        }

        // Create interval list
        let mut interval_list = vec![[0u64; 2]; number_of_parts];

        for i in 0..number_of_parts {
            if i == 0 {
                interval_list[i][0] = left_point;
                interval_list[i][1] = size_list[0] - 1 + left_point;
            } else {
                interval_list[i][0] = size_list[i - 1] + left_point;
                interval_list[i][1] = size_list[i] - 1 + left_point;
            }
        }

        interval_list
    }
}