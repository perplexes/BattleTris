h09920
s 00000/00000/00000
d R 1.2 01/10/20 13:35:39 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTStack.C
c Name history : 1 0 src/game/BTStack.C
e
s 00103/00000/00000
d D 1.1 01/10/20 13:35:38 bmc 1 0
c date and time created 01/10/20 13:35:38 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles G. Hoecker                                  */
/*    ACCT: cgh                                                 */
/*    FILE: BTStack.C                                           */
/*    ASGN: Final                                               */
/*    DATE: Mon Oct  3 16:01:59 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTStack.H"

#define RESIZE_RATIO 2

template<class T>
void BTStack<T>::tailInsert( const T& data )
{
  if ( mem_ == NULL ) {
    mem_tail_ = mem_ = (char *) malloc( mem_no_ * sizeof(T) );
    mem_end_ = mem_ + (mem_no_ * sizeof(T));
  }
  if ( mem_tail_ == mem_end_ ) {
    char *new_mem;
    new_mem = (char *)malloc( mem_no_ * RESIZE_RATIO * sizeof(T) );
    memcpy( new_mem, mem_, mem_no_ * sizeof(T) );
    mem_tail_ = mem_tail_ - mem_ + new_mem;
    mem_no_ *= RESIZE_RATIO;
    free( mem_ );
    mem_ = new_mem;
    mem_end_ = mem_ + mem_no_ * sizeof(T);
  }
//  memcpy( mem_tail_, &data, sizeof(T) );
  T *new_move = (T *) mem_tail_;
//  new_move->T();
  *new_move = data;
  mem_tail_ += sizeof(T);
  count_++;
}

template<class T>
void BTStack<T>::clear()
{
  mem_ptr_ = mem_tail_ = mem_;
  count_ = 0;
}

template<class T>
int BTStack<T>::getNext(T& data)
{
  if ( mem_ptr_ == mem_tail_ )
    return 0;
  T *da_move = (T *) mem_ptr_;
  data = *da_move;
  mem_ptr_ += sizeof(T);
  return 1;
}

template<class T>
int BTStack<T>::getPrev(T& data)
{
  if ( mem_ptr_ == mem_ )
    return 0;
  mem_ptr_ -= sizeof(T);
  T *da_move = (T *) mem_ptr_;
  data = *da_move;
  return 1;
}

template<class T>
int BTStack<T>::tailRemove(T& data)
{
  if ( mem_tail_ == mem_ )
    return 0;
  if ( mem_ptr_ == mem_tail_ )
    mem_ptr_ -= sizeof(T);
  mem_tail_ -= sizeof(T);
  count_--;
  data = *((T*)mem_tail_);
  return 1;
}

template<class T>
void BTStack<T>::operator=(BTStack<T> &other)
{
  unsigned long other_size = other.mem_tail_ - other.mem_;
  other_size /= sizeof(T);

  unsigned long new_size = mem_no_;
  while ( other_size > new_size )
    new_size *= RESIZE_RATIO;

  if (new_size > mem_no_) {
    void *new_mem;
    new_mem = malloc(new_size * sizeof(T));
    free(mem_);
    mem_ = (char *)new_mem;
    mem_no_ = new_size;
    mem_end_ = mem_ + mem_no_ * sizeof(T);
  }
  memcpy(mem_,other.mem_,other.mem_tail_-other.mem_);
  mem_tail_ = mem_ + other.mem_tail_ - other.mem_;
  mem_ptr_ = mem_;
  count_ = ( mem_tail_ - mem_ ) / sizeof(T);
}
E 1
