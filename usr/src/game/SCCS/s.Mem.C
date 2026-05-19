h44928
s 00000/00000/00000
d R 1.2 01/10/20 13:35:38 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/Mem.C
c Name history : 1 0 src/game/Mem.C
e
s 00089/00000/00000
d D 1.1 01/10/20 13:35:37 bmc 1 0
c date and time created 01/10/20 13:35:37 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: Mem.C                                               */
/*    ASGN:                                                     */
/*    DATE: Tue Oct 31 11:59:00 1995                            */
/****************************************************************/


#include "Mem.H"
#include "Hash.H"
#include <stdio.h>
#include <string.h>

#ifndef NDEBUG
#define dprintf(a) printf(a)
#else
#define dprintf(a)
#endif

#ifdef new
#undef new
#endif

struct MemRef {
  size_t size_;
  void *loc_;
  char filename_[20];
  unsigned int line_;
  MemRef() {}
  MemRef( void *loc, size_t size = 0, char *filename = "", unsigned int line = 0 ) : loc_(loc), size_(size), line_(0) {
    strncpy( filename_, filename, 20 );
  }
  inline int operator==(MemRef &other) {
    return other.loc_ == loc_;
  }
  operator unsigned() { return (unsigned)loc_; }
  MemRef &operator=(MemRef &other) {
    loc_ = other.loc_;
    size_ = other.size_;
    line_ = other.line_;
    memcpy( filename_, other.filename_, 20 );
    return *this;
  }
};

HashTable<MemRef> table_;

#ifdef new
#undef new
#endif
#ifdef delete
#undef delete
#endif

void *operator new( size_t size, char *filename, unsigned int line ) {
  static MemRef ref;
  void *mem = malloc(size);
  if (mem == 0)
    return mem;
  ref.loc_ = mem;
  ref.size_ = size;
  ref.line_ = line;
  strncpy( ref.filename_, filename, 20 );
  if (table_.add(ref))
    dprintf(("Added: %x", (char *)mem));
  else
    dprintf(("Couldn't add: %x", (char *)mem));
  return mem;
}

void operator delete( void *loc ) {
  static MemRef ref;
  ref.loc_ = loc;
  if (table_.remove(ref))
    dprintf(("Removed: %x", (char *)ref.loc_));
  else
    dprintf(("Not found: %x", (char *)ref.loc_));
  free(loc);
}

void cleanUpMem() {
  HashIter<MemRef> iter(&table_);
  MemRef ref;

  for ( iter.headJump() ; iter.peekNext(ref) ; iter.inc() )
    printf("Memory leak: %x from %s:%d (%d)\n", (char*)ref.loc_, ref.filename_,
	   ref.line_, ref.size_);
}
E 1
