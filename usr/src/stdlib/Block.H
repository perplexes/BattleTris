#ifndef BLOCK_H
#define BLOCK_H

#include "BTConfig.H"

#include <assert.h>

// This is based on Block class of the USL Standard Components

template<class T>
class Block {
private:
  T *data_;
  int size_;
   
public:
  Block (int size = 0) : size_ (size), data_(0) {
    if (size) data_ = new T[size_];
  }
  ~Block () { delete [] data_; }
  int size() { return size_;}
  inline void resize(int);
  T& operator[] (int index) { 
    assert((index < size_) && (index >= 0));
    return data_[index]; 
  }
  Block<T>& operator+= (int inc) { resize (size_ + inc); return *this; }
  Block<T>& operator-= (int dec) { resize (size_ - dec); return *this; }
  Block<T>& operator++ () { resize (size_ + 1); return *this; }
  Block<T>& operator-- () { resize (size_ - 1); return *this; } 
};

template<class T>
inline void Block<T>::resize (int size) {
  assert(size > 0);
  if (size == size_) return;
  T *new_data = new T[size];
  int stop = (size < size_) ? size : size_; 
  for (int i = 0; i < stop; i++)
    new_data[i] = data_[i];
  if (data_)
    delete [] data_;
  data_ = new_data;
  size_ = size;
}

#endif
