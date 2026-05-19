#include "BTConfig.H"

#if STDC_HEADERS
# include <ctype.h>
#else
# define isprint(x) ((x > 31) && (x < 127))
#endif

#include <stdio.h>

#define GLOB_VALIDRANGE(beg, end) \
  (isprint(beg) && isprint(end) && (end >= beg))

#define GLOB_BEGOPT	'['
#define GLOB_ENDOPT	']'
#define GLOB_NEGOPT	'!'
#define GLOB_RANGE	'-'
#define GLOB_QUOTE	'\\'
#define GLOB_ANY	'?'
#define GLOB_ALL	'*'
#define GLOB_EOS	'\0'

/*
 * Simple globbing-style pattern match function
 */

int hasglobchars(register char *pat)
{
  while(*pat != GLOB_EOS) {
    switch(*pat++) {
    case GLOB_BEGOPT:
    case GLOB_ENDOPT:
    case GLOB_NEGOPT:
    case GLOB_RANGE:
    case GLOB_QUOTE:
    case GLOB_ANY:
    case GLOB_ALL:
      return 1;
    }
  }

  return 0;
}

int globmatch(register char *text, register char *pat)
{
  register char *oldtext = text;
  register int tc = *text++;
  register int pc;

  int prev = GLOB_EOS;
  int matchrange = 0;
  int notrange = 0;

  if(*pat == GLOB_EOS)
    return tc == GLOB_EOS;

  switch(pc = *pat++) {

  case GLOB_BEGOPT:
    if(tc == GLOB_EOS)
      return 0;

    if(*pat == GLOB_NEGOPT) {
      notrange = 1;
      pat++;
    }

    pc = *pat++;

    do {
      if(pc == GLOB_RANGE && prev != GLOB_EOS && *pat != GLOB_ENDOPT) {
	pc = *pat++;

	if(pc == GLOB_QUOTE)
	  pc = *pat++;

	if(notrange) {
	  if(GLOB_VALIDRANGE(prev, pc)) {
	    if((tc < prev) || (tc > pc))
	      matchrange++;
	    else
	      return 0;
	  }
	} else {
	  if(GLOB_VALIDRANGE(prev, pc))
	    if(prev <= tc && tc <= pc)
	      matchrange++;
	}
      } else if(pc == GLOB_QUOTE) {
	pc = *pat++;
      }
	
      prev = pc;

      if(notrange) {
	if(tc != prev)
	  matchrange++;
	else
	  return 0;
      } else {
	if(tc == prev)
	  matchrange++;
      }
	
      pc = *pat++;
    } while(pc != GLOB_ENDOPT);

    if(!matchrange)
      return 0;

    return globmatch(text, pat);

  case GLOB_QUOTE:	
    pc = *pat++;

  default:
    if(pc != tc)
      return 0;

  case GLOB_ANY:
    if(tc == GLOB_EOS)
      return 0;

    return globmatch(text, pat);

  case GLOB_ALL:
    while(*pat == GLOB_ALL)
      pat++;

    if(*pat == GLOB_EOS)
      return 1;

    text = oldtext;

    while(*text) {
      if(globmatch(text, pat))
	return 1;
      text++;
    }
    
    return 0;
  }
}
