/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDB.H                                              */
/*    DATE: Sat Sep  3 17:43:12 1994                            */
/****************************************************************/

#ifndef BTDB_H
#define BTDB_H

#include "BTConfig.H"

#include <sys/stat.h>
#include <sys/types.h>

#include <errno.h>
#include <fcntl.h>

#include "BTDBErr.H"

#define BTDB_IDXSUFFIX	".idx"
#define BTDB_DATSUFFIX	".dat"

typedef unsigned long btdb_hash_t;

// Prototype for utility function to write a buffer safely
ssize_t writen(int fd, const void *buf, size_t nbytes);

class BTDBRecord;
class BTDBLock;

class BTDB {

// BattleTris Database Manager Class
// Based on the distributed database library presented in Chapter 16 of
// Advanced Programming in the UNIX Environment, by W. Richard Stevens.

protected:
  short errcode_;	// stores error code of previous command
  short valid_;		// stores error code of any error in constructor

  int idxfd_;		// index file descriptor
  int datfd_;		// data file descriptor
  int oflags_;		// flags for opening database file

  char *dbname_;	// database file name prefix
  char *idxbuf_;	// buffer for index records
  size_t idxbuflen_;	// size of idxbuf_ buffer

  off_t idxoff_;	// byte offset of index record
  off_t idxlen_;	// length of index record in bytes
  off_t datoff_;	// byte offset of data record
  off_t datlen_;	// length of data record in bytes
  off_t ptrval_;	// contents of chain ptr in index record
  off_t ptroff_;	// offset of chain ptr pointing to this index record
  off_t chnoff_;	// offset of hash chain for this index record
  off_t hshoff_;	// offset of hash table in index file

  int nhash_;		// current hash table size

  BTDBLock *findlock_;	// lock used by dbfind method

private:
  int dbfind(BTDBRecord *record, int writelock);

  btdb_hash_t dbhash(const char *key);

  off_t dbreadptr(off_t offset);
  off_t dbreadidx(off_t offset);

  void dbdelete(BTDBRecord *record);

  void dbwriterec(BTDBRecord *record, off_t offset, int whence);
  void dbwriteidx(const char *key, off_t offset, int whence, off_t ptr);
  void dbwriteptr(off_t offset, off_t ptr);

  int dbfindfree(off_t idxlen, off_t datlen);

public:
  BTDB(const char *filename, int oflags = O_RDWR);
  ~BTDB();

  int fetch(BTDBRecord *record);
  int remove(BTDBRecord *record);

  int insert(BTDBRecord *record);
  int replace(BTDBRecord *record);

  void rewind();
  int nextrec(BTDBRecord *record);

  short error() { return errcode_; }
  operator int() { return valid_; }

private:

  // Declare operator= and copy-constructor as private and leave them
  // undefined so that no one can call them

  BTDB(const BTDB& other);
  BTDB& operator=(const BTDB& other);
};

#endif
