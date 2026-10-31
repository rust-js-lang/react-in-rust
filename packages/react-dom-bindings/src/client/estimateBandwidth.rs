// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Kind;
use webapi::{Performance, navigator, performance, performance_resource_timing};

fn is_likely_static_resource(initiator_type: &str) -> bool {
    matches!(
        initiator_type,
        "css" | "script" | "font" | "img" | "image" | "input" | "link"
    )
}

js::export_default! {
    pub fn estimate_bandwidth() -> f64 {
        // Estimate the current bandwidth for downloading static resources given resources already
        // loaded.
        // $FlowFixMe[method-unbinding]
        if js::is_function(Performance::get_entries_by_type) {
            let mut count = 0;
            let mut bits = 0.0;
            let resource_entries = performance.get_entries_by_type("resource");
            let mut i = 0;
            while i < resource_entries.len() {
                let entry = performance_resource_timing::unchecked_from(resource_entries[i]);
                // $FlowFixMe[prop-missing]
                let transfer_size = entry.transfer_size();
                // $FlowFixMe[prop-missing]
                let initiator_type = entry.initiator_type();
                let duration = entry.duration();
                if !js::truthy(Some(&transfer_size))
                    || !js::truthy(Some(&duration))
                    || !is_likely_static_resource(&initiator_type)
                {
                    // Skip cached, cross-orgin entries and resources likely to be dynamically generated.
                    i += 1;
                    continue;
                }
                // Find any overlapping entries that were transferring at the same time since the total
                // bps at the time will include those bytes.
                let mut overlapping_bytes = 0.0;
                // $FlowFixMe[prop-missing]
                let parent_end_time = entry.response_end();
                let mut j = i + 1;
                while j < resource_entries.len() {
                    let overlap_entry = performance_resource_timing::unchecked_from(resource_entries[j]);
                    let overlap_start_time = overlap_entry.start_time();
                    if overlap_start_time > parent_end_time {
                        break;
                    }
                    // $FlowFixMe[prop-missing]
                    let overlap_transfer_size = overlap_entry.transfer_size();
                    // $FlowFixMe[prop-missing]
                    let overlap_initiator_type = overlap_entry.initiator_type();
                    if !js::truthy(Some(&overlap_transfer_size))
                        || !is_likely_static_resource(&overlap_initiator_type)
                    {
                        // Skip cached, cross-orgin entries and resources likely to be dynamically generated.
                        j += 1;
                        continue;
                    }
                    // $FlowFixMe[prop-missing]
                    let overlap_end_time = overlap_entry.response_end();
                    let overlap_factor = if overlap_end_time < parent_end_time {
                        1.0
                    } else {
                        (parent_end_time - overlap_start_time) / (overlap_end_time - overlap_start_time)
                    };
                    overlapping_bytes += overlap_transfer_size * overlap_factor;
                    j += 1;
                }
                // Skip past any entries we already considered overlapping. Otherwise we'd have to go
                // back to consider previous entries when we then handled them.
                i = j;

                let bps = ((transfer_size + overlapping_bytes) * 8.0) / (entry.duration() / 1000.0);
                bits += bps;
                count += 1;
                if count > 10 {
                    // We have enough to get an average.
                    break;
                }
            }
            if count > 0 {
                return bits / f64::from(count) / 1e6;
            }
        }

        // Fallback to the navigator.connection estimate if available
        // $FlowFixMe[prop-missing]
        if js::truthy(js::get(navigator, "connection")) {
            // $FlowFixMe[incompatible-use]
            let downlink = js::get(js::get(navigator, "connection"), "downlink");
            if let Kind::Number(downlink) = js::classify(downlink) {
                return downlink;
            }
        }

        // Otherwise, use a default of 5mbps to compute heuristics.
        // This can happen commonly in Safari if all static resources and images are loaded
        // cross-orgin.
        5.0
    }
}
