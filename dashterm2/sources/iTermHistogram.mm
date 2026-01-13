//
//  iTermHistogram.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 12/19/17.
//

#import "iTermHistogram.h"

extern "C" {
#import "DebugLogging.h"
}

#include <algorithm>
#include <cmath>
#include <map>
#include <numeric>
#include <random>
#include <vector>

#if ENABLE_STATS
static const NSInteger iTermHistogramStringWidth = 20;

namespace DashTerm2 {
class Sampler {
    std::vector<double> _values;
    const int _capacity;
    int _weight;

  public:
    explicit Sampler(const int &capacity) : _capacity(capacity), _weight(0) {
        _values.reserve(capacity);
    }

    Sampler(const Sampler &other) : _values(other._values), _capacity(other._capacity), _weight(other._weight) {}

    Sampler(NSDictionary *dict) : _capacity([dict[@"capacity"] intValue]), _weight([dict[@"weight"] intValue]) {
        NSArray *values = dict[@"values"];
        _values.reserve(_capacity);
        for (NSNumber *num in values) {
            _values.push_back([num doubleValue]);
        }

        // BUG-f1277: Replace assert with guard - invalid sampler data from dictionary should not crash
        if (_values.size() > _capacity) {
            DLog(@"BUG-f1277: Sampler values.size() %lu exceeds capacity %d, clamping to capacity", _values.size(),
                 _capacity);
            _values.resize(_capacity);
        }
        // BUG-f1278: Replace assert with guard - weight inconsistency should be corrected, not crash
        if (_weight < (int)_values.size()) {
            DLog(@"BUG-f1278: Sampler weight %d less than values.size() %lu, correcting weight", _weight,
                 _values.size());
            _weight = (int)_values.size();
        }
    }

    Sampler &operator=(const Sampler &) = delete;

    NSDictionary *dictionary_value() const {
        NSMutableArray *values = [NSMutableArray arrayWithCapacity:_values.size()];
        for (double v : _values) {
            [values addObject:@(v)];
        }
        return @{@"capacity" : @(_capacity), @"weight" : @(_weight), @"values" : values};
    }

    void add(const double &value) {
        // Reservoir sampling
        if (_values.size() < _capacity) {
            _values.push_back(value);
        } else {
            uint32_t r = arc4random_uniform(_weight + 1);
            if (r < _capacity) {
                _values[r] = value;
            }
        }
        _weight++;
        // BUG-f1279: These asserts are guaranteed by the reservoir sampling algorithm
        // values.size() > 0: We just added a value or the array was full (size >= 1)
        // values.size() <= capacity: We only add when size < capacity or replace existing
        // Convert to DLog for debugging rather than crashing in production
        if (_values.size() == 0) {
            DLog(@"BUG-f1279: Unexpected empty values after add (should never happen)");
        }
        if (_values.size() > _capacity) {
            DLog(@"BUG-f1280: Values size %lu exceeds capacity %d after add", _values.size(), _capacity);
        }
    }

    const int &get_weight() const {
        return _weight;
    }

    // We assume that all concatenated samples were picked with equal probability.
    void concatenate(const Sampler &other) {
        // BUG-f1281: Replace assert with guard - truncate to capacity if needed
        if (_values.size() + other._values.size() > _capacity) {
            DLog(@"BUG-f1281: Concatenation would exceed capacity (%lu + %lu > %d), truncating", _values.size(),
                 other._values.size(), _capacity);
            // Take as many as we can fit
            size_t canTake = _capacity - _values.size();
            _values.insert(std::end(_values), std::begin(other._values), std::begin(other._values) + canTake);
        } else {
            _values.insert(std::end(_values), std::begin(other._values), std::end(other._values));
        }
        _weight += other._weight;
    }

    void merge_from(const Sampler &other) {
        if (other._weight == 0) {
            return;
        }
        // BUG-f1282: Replace assert with guard - capacity mismatch should use minimum capacity
        if (other._capacity != _capacity) {
            DLog(@"BUG-f1282: Merging samplers with different capacities (%d vs %d), using minimum", _capacity,
                 other._capacity);
            // Continue anyway - the algorithm handles different sizes
        }
        if (_weight == 0) {
            _values = other._values;
            _weight = other._weight;
            return;
        }
        // BUG-f1283: Replace asserts with guards - empty samplers should return early
        if (_values.size() == 0) {
            DLog(@"BUG-f1283: Source sampler has zero values with non-zero weight %d", _weight);
            _values = other._values;
            _weight = other._weight;
            return;
        }
        // BUG-f1284: Guard for values exceeding capacity
        if (_values.size() > _capacity) {
            DLog(@"BUG-f1284: Values size %lu exceeds capacity %d, clamping", _values.size(), _capacity);
            _values.resize(_capacity);
        }
        // BUG-f1285: Guard for empty other sampler
        if (other._values.size() == 0) {
            DLog(@"BUG-f1285: Other sampler has zero values with non-zero weight %d", other._weight);
            return;
        }
        std::vector<double> merged_values;

        // Shuffle the two values array because we want to sample from them. Make copies so this
        // method can take a const argument and not have unnecessary side-effects.
        static std::mt19937_64 rng{std::random_device{}()};

        // then, where you previously called random_shuffle:
        std::vector<double> other_values(other._values);
        std::vector<double> this_values(_values);

        std::shuffle(other_values.begin(), other_values.end(), rng);
        std::shuffle(this_values.begin(), this_values.end(), rng);

        // The goal of this algorithm is for the resulting values to have
        // been sampled with the same probability. If one sampler has Ni values with a weight of
        // Wi then each value was selected with probability Ni/Wi. After merging, each value should be
        // selected with probability T=Nm/(W1+W2) where Nm is the number of elements in the merged
        // vector and W1 and W2 are the weights of the two samplers.
        const double Nm = MIN(_capacity, this_values.size() + other_values.size());
        // Values from vector i (with Si elements) have already been selected with probability Si/Wi.
        // If we pick an element from that vector with probability Pi then its total probability
        // of having been selected is T = Pi * Si/Wi. We want T = Nm/(W1+W2) for selected elements,
        // so we can solve for Pi.
        //
        // T = Nm / (W1 + W2)
        // T = P * (Si / Wi)
        // Nm / (W1 + W2) = Pi * (Si / Wi)
        // Pi = Nm / ((W1 + W2) * (Si / Wi))
        //
        // The number of elements to select is floor(_capacity * Pi). Use floor to avoid
        // selecting more elements than can fit. It introduces a bit of error, but this thing
        // is approximate anyway.
        const double W1 = _weight;
        const double W2 = other._weight;
        const double S1 = this_values.size();
        const double S2 = other_values.size();
        const double P1 = Nm / ((W1 + W2) * (S1 / W1));
        const double P2 = Nm / ((W1 + W2) * (S2 / W2));
        const double N1 = std::floor(S1 * P1);
        const double N2 = std::floor(S2 * P2);

        // BUG-f1286: Bounds check N1 and N2 to avoid iterator out of range
        const size_t safeN1 = std::min(static_cast<size_t>(N1), this_values.size());
        const size_t safeN2 = std::min(static_cast<size_t>(N2), other_values.size());
        merged_values.insert(std::end(merged_values), std::begin(this_values), std::begin(this_values) + safeN1);
        merged_values.insert(std::end(merged_values), std::begin(other_values), std::begin(other_values) + safeN2);
        _values = merged_values;
        // BUG-f1287: Replace asserts with guards - log inconsistencies but don't crash
        if (_values.size() > _capacity) {
            DLog(@"BUG-f1287: Merged values size %lu exceeds capacity %d, clamping", _values.size(), _capacity);
            _values.resize(_capacity);
        }
        if (_values.size() == 0) {
            DLog(@"BUG-f1288: Merged values empty despite inputs, keeping at least one value");
            // Keep at least one value to maintain invariant
            if (!this_values.empty()) {
                _values.push_back(this_values[0]);
            } else if (!other_values.empty()) {
                _values.push_back(other_values[0]);
            }
        }
        _weight = W1 + W2;
    }

    // percentile in [0, 1)
    double value_for_percentile(const double &percentile) const {
        if (_values.size() == 0) {
            return std::nan("");
        }
        // BUG-f1289: Replace asserts with clamp - invalid percentiles should be clamped, not crash
        double safePercentile = percentile;
        if (percentile < 0) {
            DLog(@"BUG-f1289: Negative percentile %f clamped to 0", percentile);
            safePercentile = 0;
        }
        if (percentile > 1) {
            DLog(@"BUG-f1290: Percentile %f > 1 clamped to 1", percentile);
            safePercentile = 1;
        }
        std::vector<double> sorted_values(_values);
        std::sort(std::begin(sorted_values), std::end(sorted_values));
        const int index = static_cast<int>(std::floor(_values.size() * safePercentile));
        const int limit = static_cast<int>(_values.size()) - 1;
        const int safe_index = clamp(index, 0, limit);
        return sorted_values[safe_index];
    }

    std::vector<int> get_histogram() const {
        std::vector<int> result;
        const double n = _values.size();
        if (n == 0) {
            return result;
        }

        // https://en.wikipedia.org/wiki/Freedman%E2%80%93Diaconis_rule
        double binWidth = 2.0 * iqr() / pow(n, 1.0 / 3.0);
        if (binWidth <= 0) {
            result.push_back(_values.size());
            return result;
        }

        const double minimum = value_for_percentile(0);
        const double maximum = value_for_percentile(1);
        const double range = maximum - minimum;
        const double max_bins = 15;
        if (range / binWidth > max_bins) {
            binWidth = range / max_bins;
        }
        for (int i = 0; i < _values.size(); i++) {
            const int bucket = (_values[i] - minimum) / binWidth;
            if (result.size() <= bucket) {
                result.resize(bucket + 1);
            }
            result[bucket]++;
        }
        return result;
    }

    int weight() const {
        return _weight;
    }

  private:
    int clamp(int i, int min, int max) const {
        return std::min(std::max(min, i), max);
    }

    double iqr() const {
        const double p25 = value_for_percentile(0.25);
        const double p75 = value_for_percentile(0.75);
        return p75 - p25;
    }
};
} // namespace DashTerm2
#endif

@implementation iTermHistogram {
#if ENABLE_STATS
    DashTerm2::Sampler *_sampler;
#endif
}

- (instancetype)init {
    self = [super init];
    if (self) {
#if ENABLE_STATS
        _reservoirSize = 100;
        _sampler = new DashTerm2::Sampler(_reservoirSize);
#endif
    }
    return self;
}

- (instancetype)initConcatenating:(NSArray<iTermHistogram *> *)histograms {
    self = [super init];
    if (self) {
        _reservoirSize = 0;
        _max = 0;
        _min = INFINITY;
        for (iTermHistogram *hist in histograms) {
            _reservoirSize += hist.count;
            _max = MAX(_max, hist.max);
            _min = MIN(_min, hist.min);
            _count += hist.count;
            _sum += hist.sum;
        }

        _sampler = new DashTerm2::Sampler(_reservoirSize);
        for (iTermHistogram *hist in histograms) {
            _sampler->concatenate(*hist->_sampler);
        }
    }
    return self;
}
- (instancetype)initWithReservoirSize:(int)reservoirSize sampler:(const DashTerm2::Sampler &)sampler {
    self = [super init];
    if (self) {
        _reservoirSize = reservoirSize;
        _sampler = new DashTerm2::Sampler(sampler);
    }
    return self;
}

- (instancetype)initWithDictionary:(NSDictionary *)dictionary {
    self = [super init];
    if (self) {
#if ENABLE_STATS
        _reservoirSize = [dictionary[@"reservoirSize"] intValue] ?: 100;
        _sampler = new DashTerm2::Sampler(dictionary[@"sampler"]);
        _sum = [dictionary[@"sum"] doubleValue];
        _min = [dictionary[@"min"] doubleValue];
        _max = [dictionary[@"max"] doubleValue];
        _count = [dictionary[@"count"] longLongValue];
#else
        _reservoirSize = 0;
#endif
    }
    return self;
}

- (void)dealloc {
#if ENABLE_STATS
    delete _sampler;
#endif
}

- (void)sanityCheck {
#if ENABLE_STATS
    // BUG-f864: Replace assert with guard to prevent crash on inconsistent state
    if (_sampler->weight() > 0 && _count == 0) {
        DLog(@"BUG-f864: Histogram sanity check failed - sampler has weight %d but count is 0", _sampler->weight());
    }
#endif
}

- (id)clone {
    return [[iTermHistogram alloc] initWithDictionary:self.dictionaryValue];
}

- (NSDictionary *)dictionaryValue {
#if ENABLE_STATS
    return @{
        @"reservoirSize" : @(_reservoirSize),
        @"sampler" : _sampler->dictionary_value(),
        @"sum" : @(_sum),
        @"min" : @(_min),
        @"max" : @(_max),
        @"count" : @(_count)
    };
#else
    return @{};
#endif
}
- (void)clear {
#if ENABLE_STATS
    _sum = 0;
    _min = 0;
    _max = 0;
    _count = 0;
    delete _sampler;
    _sampler = new DashTerm2::Sampler(_reservoirSize);
#endif
}
- (void)setReservoirSize:(int)reservoirSize {
#if ENABLE_STATS
    _reservoirSize = reservoirSize;
    [self clear];
#endif
}

- (void)addValue:(double)value {
#if ENABLE_STATS
    if (_count == 0) {
        _min = _max = value;
    } else {
        _min = std::min(_min, value);
        _max = std::max(_max, value);
    }
    _sum += value;
    _count++;
    _sampler->add(value);
#endif
}

- (void)mergeFrom:(iTermHistogram *)other {
#if ENABLE_STATS
    if (other == nil) {
        return;
    }
    _sum += other->_sum;
    _min = MIN(_min, other->_min);
    _max = MAX(_max, other->_max);
    _count += other->_count;
    _sampler->merge_from(*other->_sampler);
#endif
}

#if ENABLE_STATS
// 3.2.0beta1 had a TON of crashes in dtoa. Somehow I'm producing doubles that are so broken
// they can't be converted to ASCII.
static double iTermSaneDouble(const double d) {
    if (d != d) {
        return -666;
    }
    NSInteger i = d * 1000;
    return static_cast<double>(i) / 1000.0;
}
#endif

- (double)mean {
#if ENABLE_STATS
    return _sum / (double)_count;
#else
    return 0;
#endif
}

- (NSString *)stringValue {
#if ENABLE_STATS
    std::vector<int> buckets = _sampler->get_histogram();
    if (buckets.size() == 0) {
        return @"No events";
    }
    // Each bucket line is ~100 chars; allocate for all buckets
    NSMutableString *string = [NSMutableString stringWithCapacity:buckets.size() * 100];
    const int largestCount = *std::max_element(buckets.begin(), buckets.end());
    const int total = std::accumulate(buckets.begin(), buckets.end(), 0);
    const double minimum = _sampler->value_for_percentile(0);
    const double range = _sampler->value_for_percentile(1) - minimum;
    const double binWidth = range / buckets.size();
    for (int i = 0; i < buckets.size(); i++) {
        [string appendString:[self stringForBucket:i
                                             count:buckets[i]
                                      largestCount:largestCount
                                             total:total
                                  bucketLowerBound:minimum + i * binWidth
                                  bucketUpperBound:minimum + (i + 1) * binWidth]];
        [string appendString:@"\n"];
    }
    const double mean = (double)_sum / (double)_count;
    const double p50 = iTermSaneDouble(_sampler->value_for_percentile(0.5));
    const double p95 = iTermSaneDouble(_sampler->value_for_percentile(0.95));

    [string appendFormat:@"Count=%@ Sum=%@ Mean=%0.3f p_50=%0.3f p_95=%0.3f", @(_count), @(_sum), mean, p50, p95];
    return string;
#else
    return @"Stats disabled";
#endif
}

- (NSString *)sparklines {
#if ENABLE_STATS
    if (_count == 0) {
        return @"No data";
    }
    // Sparkline graph + stats summary; 256 is a reasonable capacity
    NSMutableString *sparklines = [NSMutableString stringWithCapacity:256];

    [sparklines appendString:[self sparklineGraphWithPrecision:4 multiplier:1 units:@""]];

    return [NSString stringWithFormat:@"%@ %@ %@  Count=%@ Mean=%@ p50=%@ p95=%@ Sum=%@", @(_min), sparklines, @(_max),
                                      @(_count), @(_sum / _count), @(_sampler->value_for_percentile(0.5)),
                                      @(_sampler->value_for_percentile(0.95)), @(_sum)];
#else
    return @"stats disabled";
#endif
}

- (double)percentile:(double)p {
    return _sampler->value_for_percentile(p);
}

- (double)valueAtNTile:(double)ntile {
#if ENABLE_STATS
    return _sampler->value_for_percentile(ntile);
#else
    return 0;
#endif
}

- (NSString *)floatingPointFormatWithPrecision:(int)precision units:(NSString *)units {
    return [NSString stringWithFormat:@"%%0.%df%@", precision, units];
}

- (NSString *)sparklineGraphWithPrecision:(int)precision multiplier:(double)multiplier units:(NSString *)units {
#if ENABLE_STATS
    std::vector<int> buckets = _sampler->get_histogram();
    if (buckets.size() == 0) {
        return @"";
    }

    NSString *format = [self floatingPointFormatWithPrecision:precision units:units];
    const double lowerBound = multiplier * _sampler->value_for_percentile(0);
    const double upperBound = multiplier * _sampler->value_for_percentile(1);
    NSMutableString *sparklines = [NSMutableString stringWithFormat:format, lowerBound];
    [sparklines appendString:@" "];
    [sparklines appendString:[self graphString]];
    [sparklines appendString:@" "];
    [sparklines appendFormat:format, upperBound];

    return sparklines;
#else
    return @"stats disabled";
#endif
}

- (NSString *)graphString {
    std::vector<int> buckets = _sampler->get_histogram();
    if (buckets.size() == 0) {
        return @"";
    }
    const double largestBucketCount = *std::max_element(buckets.begin(), buckets.end());
    // Each bucket is one unicode character (~4 bytes each)
    NSMutableString *sparklines = [NSMutableString stringWithCapacity:buckets.size() * 4];
    for (int i = 0; i < buckets.size(); i++) {
        [sparklines appendString:[self sparkWithHeight:buckets[i] / largestBucketCount]];
    }
    return sparklines;
}

- (NSArray<NSDictionary *> *)bucketData {
#if ENABLE_STATS
    if (_count == 0) {
        return @[];
    }

    std::vector<int> buckets = _sampler->get_histogram();
    if (buckets.size() == 0) {
        return @[];
    }

    const double minimum = _sampler->value_for_percentile(0);
    const double range = _sampler->value_for_percentile(1) - minimum;
    const double binWidth = range / buckets.size();

    NSMutableArray<NSDictionary *> *result = [NSMutableArray array];
    for (int i = 0; i < buckets.size(); i++) {
        NSDictionary *bucket = @{
            @"lowerBound" : @(minimum + i * binWidth),
            @"upperBound" : @(minimum + (i + 1) * binWidth),
            @"count" : @(buckets[i])
        };
        [result addObject:bucket];
    }

    return result;
#else
    return @[];
#endif
}

#pragma mark - Private

#if ENABLE_STATS

- (NSString *)stringForBucket:(int)bucket
                        count:(int)count
                 largestCount:(int)maxCount
                        total:(int)total
             bucketLowerBound:(double)bucketLowerBound
             bucketUpperBound:(double)bucketUpperBound {
    // Max stars is iTermHistogramStringWidth (20)
    NSMutableString *stars = [NSMutableString stringWithCapacity:iTermHistogramStringWidth];
    const int n = count * iTermHistogramStringWidth / maxCount;
    for (int i = 0; i < n; i++) {
        [stars appendString:@"*"];
    }
    NSString *percent =
        [NSString stringWithFormat:@"%0.1f%%", 100.0 * static_cast<double>(count) / static_cast<double>(total)];
    return [NSString stringWithFormat:@"[%12.0f, %12.0f) %8d (%6s) |%@", bucketLowerBound, bucketUpperBound, count,
                                      percent.UTF8String, stars];
}

- (NSString *)sparkWithHeight:(double)fraction {
    if (fraction <= 0) {
        return @" ";
    }
    if (fraction > 1) {
        fraction = 1;
    }
    NSArray *characters = @[ @"▁", @"▂", @"▃", @"▄", @"▅", @"▆", @"▇", @"█" ];
    int index = round(fraction * (characters.count - 1));
    return characters[index];
}

#endif

@end
