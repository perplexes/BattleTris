h38637
s 00000/00000/00000
d R 1.2 01/10/20 13:35:37 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTMovePath.C
c Name history : 1 0 src/game/BTMovePath.C
e
s 00101/00000/00000
d D 1.1 01/10/20 13:35:36 bmc 1 0
c date and time created 01/10/20 13:35:36 by bmc
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
/*    FILE: BTMovePath.C                                        */
/*    ASGN: Final                                               */
/*    DATE: Mon Oct  3 16:01:59 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTMovePath.H"

#define RESIZE_RATIO 2

template<class T>
T *BTList<T>::tailInsert( int x, int y, BT_MOVE_DIR dir )
{
  if ( move_mem_ == NULL ) {
    move_mem_tail_ = move_mem_ = (char *) malloc( move_mem_no_ * sizeof(T) );
    move_mem_end_ = move_mem_ + (move_mem_no_ * sizeof(T));
  }
  if ( move_mem_tail_ == move_mem_end_ ) {
    char *new_mem;
    new_mem = (char *)malloc( move_mem_no_ * RESIZE_RATIO * sizeof(T) );
    memcpy( new_mem, move_mem_, move_mem_no_ * sizeof(T) );
    move_mem_tail_ = move_mem_tail_ - move_mem_ + new_mem;
    move_mem_no_ *= RESIZE_RATIO;
    free( move_mem_ );
    move_mem_ = new_mem;
    move_mem_end_ = move_mem_ + move_mem_no_ * sizeof(T);
  }
  T *new_move = (T *) move_mem_tail_;
  new_move->x_ = x;
  new_move->y_ = y;
  new_move->dir_ = dir;
  move_mem_tail_ += sizeof(T);
  count_++;
  return new_move;
}

template<class T>
void BTList<T>::clear()
{
  move_mem_ptr_ = move_mem_tail_ = move_mem_;
  count_ = 0;
}

template<class T>
T *BTList<T>::getNext()
{
  if ( move_mem_ptr_ == move_mem_tail_ )
    return NULL;
  T *da_move = (T *) move_mem_ptr_;
  move_mem_ptr_ += sizeof(T);
  return da_move;
}

template<class T>
T *BTList<T>::getPrev()
{
  if ( move_mem_ptr_ == move_mem_ )
    return NULL;
  move_mem_ptr_ -= sizeof(T);
  T *da_move = (T *) move_mem_ptr_;
  return da_move;
}

template<class T>
T * BTList<T>::TailRemove()
{
  if ( move_mem_tail_ == move_mem_ )
    return 0;
  if ( move_mem_ptr_ == move_mem_tail_ )
    move_mem_ptr_ -= sizeof(T);
  move_mem_tail_ -= sizeof(T);
  count_--;
  return (T*) move_mem_tail_;
}

template<class T>
void BTList<T>::operator=(BTList<T> &other)
{
  int other_size = other.move_mem_tail_ - other.move_mem_;
  other_size /= sizeof(T);

  int new_size = move_mem_no_;
  while ( other_size > new_size )
    new_size *= RESIZE_RATIO;

  if (new_size > move_mem_no_) {
    void *new_mem;
    new_mem = malloc(new_size * sizeof(T));
    free(move_mem_);
    move_mem_ = new_mem;
    move_mem_no_ = new_size;
    move_mem_end_ = move_mem_ + move_mem_no_ * sizeof(T);
  }
  memcpy(move_mem_,other.move_mem_,other.move_mem_tail_-other.move_mem_);
  move_mem_tail_ = move_mem_ + other.move_mem_tail_other.move_mem_;
  move_mem_ptr_ = move_mem_;
  count_ = ( move_mem_tail_ - move_mem_ ) / sizeof(T);
}
E 1
