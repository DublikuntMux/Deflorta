#pragma once

#include "Base/Random.hpp"

#include <vector>
#include <algorithm>
#include <numeric>
#include <stdexcept>

template <typename T>
class WeightedArray
{
public:
    struct Entry
    {
        T value;
        double weight;
    };

    void add(T value, double weight)
    {
        if (weight <= 0.0) return;
        entries_.push_back({std::move(value), weight});
        needsUpdate_ = true;
    }

    void reserve(std::size_t capacity)
    {
        entries_.reserve(capacity);
        prefixSums_.reserve(capacity);
    }

    const T& pick() const
    {
        if (entries_.empty())
            throw std::runtime_error("WeightedArray is empty!");

        if (needsUpdate_)
            recomputePrefix();

        const double r = Random::UniformDouble(0.0, totalWeight_);

        const auto it = std::ranges::upper_bound(prefixSums_, r);
        const std::size_t idx = std::distance(prefixSums_.begin(), it);

        return entries_[idx].value;
    }

    [[nodiscard]] std::size_t size() const noexcept { return entries_.size(); }
    [[nodiscard]] bool empty() const noexcept { return entries_.empty(); }

    [[nodiscard]] double totalWeight() const
    {
        if (needsUpdate_)
            recomputePrefix();
        return totalWeight_;
    }

    void clear() noexcept
    {
        entries_.clear();
        prefixSums_.clear();
        totalWeight_ = 0.0;
        needsUpdate_ = false;
    }

private:
    void recomputePrefix() const
    {
        prefixSums_.resize(entries_.size());
        totalWeight_ = 0.0;

        for (std::size_t i = 0; i < entries_.size(); ++i)
        {
            totalWeight_ += entries_[i].weight;
            prefixSums_[i] = totalWeight_;
        }
        needsUpdate_ = false;
    }

    std::vector<Entry> entries_;
    mutable std::vector<double> prefixSums_;
    mutable double totalWeight_ = 0.0;
    mutable bool needsUpdate_ = false;
};
