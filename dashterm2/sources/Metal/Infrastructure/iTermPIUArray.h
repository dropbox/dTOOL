//
//  iTermPIUArray.h
//  DashTerm2
//
//  Created by George Nachman on 12/19/17.
//

#import <vector>

namespace DashTerm2 {
    // A PIUArray is an array of arrays of PIU structs. This avoids giant allocations.
    // It is append-only.
    template<class T>
    class PIUArray {
    public:
        // Maximum number of PIUs in one segment.
        const static size_t DEFAULT_CAPACITY = 1024;

        PIUArray() : _capacity(DEFAULT_CAPACITY), _size(0) {
            _arrays.resize(1);
            _arrays.back().reserve(_capacity);
        }

        explicit PIUArray(size_t capacity) : _capacity(capacity), _size(0) {
            _arrays.resize(1);
            _arrays.back().reserve(_capacity);
        }

        T *get_next() {
            if (_arrays.back().size() == _capacity) {
                _arrays.resize(_arrays.size() + 1);
                _arrays.back().reserve(_capacity);
            }

            std::vector<T> &array = _arrays.back();
            array.resize(array.size() + 1);
            _size++;
            return &array.back();
        }

        // BUG-1107, BUG-1108: Added bounds validation for get() methods
        T &get(const size_t &segment, const size_t &index) {
            if (segment >= _arrays.size()) {
                static T dummy{};
                return dummy;
            }
            if (index >= _arrays[segment].size()) {
                static T dummy{};
                return dummy;
            }
            return _arrays[segment][index];
        }

        T &get(const size_t &index) {
            if (index >= _size) {
                static T dummy{};
                return dummy;
            }
            return _arrays[index / _capacity][index % _capacity];
        }

        void push_back(const T &piu) {
            memmove(get_next(), &piu, sizeof(piu));
        }

        size_t get_number_of_segments() const {
            return _arrays.size();
        }

        // BUG-1107: Added bounds validation for segment accessors
        const T *start_of_segment(const size_t segment) const {
            if (segment >= _arrays.size() || _arrays[segment].empty()) {
                return nullptr;
            }
            return &_arrays[segment][0];
        }

        size_t size_of_segment(const size_t segment) const {
            if (segment >= _arrays.size()) {
                return 0;
            }
            return _arrays[segment].size();
        }

        const size_t &size() const {
            return _size;
        }

        // Pre-allocate space for at least `count` PIUs to avoid segment allocations during rendering.
        // Phase 1C optimization: Pre-allocation based on previous frame's usage.
        void reserve(size_t count) {
            if (count <= _capacity) {
                // First segment already has enough capacity
                return;
            }
            // Calculate how many segments we need
            size_t segments_needed = (count + _capacity - 1) / _capacity;
            if (segments_needed > _arrays.size()) {
                _arrays.reserve(segments_needed);
                // Pre-allocate additional segments
                while (_arrays.size() < segments_needed) {
                    _arrays.resize(_arrays.size() + 1);
                    _arrays.back().reserve(_capacity);
                }
            }
        }

        // Clear all data but keep allocated memory for reuse.
        // Phase 1C optimization: Avoids deallocation/reallocation between frames.
        void clear() {
            for (auto &array : _arrays) {
                array.clear();
            }
            // Keep first segment, release others to avoid unbounded growth
            if (_arrays.size() > 1) {
                _arrays.resize(1);
            }
            _size = 0;
        }

    private:
        const size_t _capacity;
        size_t _size;
        std::vector<std::vector<T>> _arrays;
    };
}
