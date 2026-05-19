#ifndef BTCONFIGFILE_H
#define BTCONFIGFILE_H

#include "BTConfig.H"

enum {
  BTCONFIGFILE_OK,
  BTCONFIGFILE_BADFILE,
  BTCONFIGFILE_MEMERR,
  BTCONFIGFILE_CONFERR
};

class BTConfigFile {

protected:
  int verifyfile(char **bufaddr, const char *token, const char *file, int line);
  int verifydir(char **bufaddr, const char *token, const char *file, int line);

  char *datadir_;
  char *logsdir_;
  char *pipedir_;
  char *slvpath_;
  char *audiodir_;
  char *artdir_;
  int status_;

public:
  BTConfigFile(const char *configfile);
  ~BTConfigFile();

  const char *datadir() { return datadir_; }
  const char *logsdir() { return logsdir_; }
  const char *pipedir() { return pipedir_; }
  const char *slvpath() { return slvpath_; }
  const char *audiodir() { return audiodir_; }
  const char *artdir() { return artdir_; }

  int status() { return status_; }
  int operator!() { return status_ != BTCONFIGFILE_OK; }
  operator int() { return status_ == BTCONFIGFILE_OK; }

  BTConfigFile(const BTConfigFile& other);
  BTConfigFile& operator=(const BTConfigFile& other);
};

#endif
