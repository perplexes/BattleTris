h33705
s 00000/00000/00000
d R 1.2 01/10/20 13:34:51 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/ParsedFile.C
c Name history : 1 0 src/db/ParsedFile.C
e
s 00063/00000/00000
d D 1.1 01/10/20 13:34:50 bmc 1 0
c date and time created 01/10/20 13:34:50 by bmc
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "ParsedFile.H"

int ParsedFile::parseline(char comment)
{
  char *pptr = (char *) 0, *tok = (char *) 0;
  register char *cptr;
  int inquote = 0;
  int tmp;

  ntokens_ = 0;
  tokidx_ = 0;

  if(is_.fail() || is_.eof())
    return 0;

  is_.getline(buf_, sizeof(buf_) - 1, '\n');

  for(cptr = buf_; *cptr != '\0'; cptr++) {
    if(*cptr == comment) {
      *cptr = '\0';
      break;
    }
  }

  for(cptr = buf_; *cptr != '\0'; pptr = cptr++) {
    switch(*cptr) {
      case '\t':
      case ' ':
        if(inquote) {
          if(tok == (char *) 0)
            tok = cptr;
        } else {
          *cptr = '\0';
          if(tok) {
            tokens_[ntokens_++] = tok;
            tok = (char *) 0;
          }
        }
        break;

      case '"':
        if((inquote = 1 - inquote) == 0) {
          *cptr = '\0';
          if(tok) {
            tokens_[ntokens_++] = tok;
            tok = (char *) 0;
          } 
        }
        break;
 
      default:
        if(tok == (char *) 0)
          tok = cptr;
    }
  }

  if(tok)
    tokens_[ntokens_++] = tok;

  tokens_[ntokens_] = (char *) 0;
  return 1;
}
E 1
