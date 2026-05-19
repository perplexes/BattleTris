h09527
s 00000/00000/00000
d R 1.2 01/10/20 13:34:50 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/ParsedFile.H
c Name history : 1 0 src/db/ParsedFile.H
e
s 00042/00000/00000
d D 1.1 01/10/20 13:34:49 bmc 1 0
c date and time created 01/10/20 13:34:49 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef PARSEDFILE_H
#define PARSEDFILE_H

#include "BTConfig.H"

#include <fstream.h>

class ParsedFile {

  enum {
    PF_LINEBUF = 2048
  };

protected:
  char *tokens_[PF_LINEBUF];
  char buf_[PF_LINEBUF];
  int ntokens_;
  int tokidx_;
  int eof_;
  ifstream is_;

public:
  ParsedFile(const char *pathname)
  : ntokens_(0), tokidx_(0), is_(pathname), eof_(0) {}

  ~ParsedFile() {}

  int parseline(char comment = '#');

  char *token() {
    if(tokidx_ < ntokens_)
      return tokens_[tokidx_++];
    return (char *) 0;
  }

  int ntokens() { return ntokens_; }

  int fail() { return is_.fail(); }
  int eof() { return is_.eof() || eof_; }
};

#endif
E 1
