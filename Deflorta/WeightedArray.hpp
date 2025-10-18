#pragma once

#include <iostream>
#include <vector>
#include <numeric>

#include "Base/Random.hpp"

template <typename T>
class WeightedArray
{
public:
    struct Entry
    {
        T value;
        double weight;
    };

    void add(const T& value, double weight)
    {
        if (weight <= 0.0) return;
        entries_.push_back({value, weight});
        recomputePrefix();
    }

    const T& pick() const
    {
        if (entries_.empty())
            throw std::runtime_error("WeightedArray is empty!");

        const double r = Random::UniformDouble(0.0, totalWeight_);
        const auto it = std::ranges::upper_bound(prefixSums_, r);
        std::size_t idx = std::distance(prefixSums_.begin(), it);

        return entries_[idx].value;
    }

    double totalWeight() const noexcept { return totalWeight_; }

private:
    std::vector<Entry> entries_;
    std::vector<double> prefixSums_;
    double totalWeight_ = 0.0;

    void recomputePrefix();
};
