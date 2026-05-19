h34497
s 00000/00006/00333
d D 1.5 01/10/23 00:05:28 bmc 6 5
c 1000018 "mofo" is not a filename
e
s 00221/00218/00118
d D 1.4 01/10/22 17:56:47 ahl 5 4
c 1000013 props for airslides
e
s 00001/00001/00335
d D 1.3 01/10/21 19:25:12 bmc 4 3
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00047/00040/00289
d D 1.2 01/10/21 01:52:48 bmc 3 1
c 1000009 shouldn't core dump when sound files not found
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:33 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTSoundManager.C
c Name history : 1 0 src/game/BTSoundManager.C
e
s 00329/00000/00000
d D 1.1 01/10/20 13:35:32 bmc 1 0
c date and time created 01/10/20 13:35:32 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTSound.C                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 15:19:33 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
#include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

// releng note -- this needs to be autoconf'ed --mws
#include <dirent.h>

#include "BattleTris.H"

#include "BTDebug.H"
#include "BTWeapon.H"
#include "BTDirs.H"
#include "BTSoundManager.H"

D 3
void BTSoundManager::read_dir (char *dir_name, Block<BTSoundFile *> *block) {
  DIR *dir;
  struct dirent *dirent;
E 3
I 3
void
D 4
BTSoundManager::read_dir(char *dir_name, Block<BTSoundFile *> *block)
E 4
I 4
BTSoundManager::read_dir(const char *dir_name, Block<BTSoundFile *> *block)
E 4
{
	DIR *dir;
	struct dirent *dirent;
	char sound_dir[BT_MAX_SND_FILENAME];
	char name[BT_MAX_SND_FILENAME];
E 3

D 3
  char sound_dir[BT_MAX_SND_FILENAME];
  char name[BT_MAX_SND_FILENAME];
E 3
I 3
	strcpy(sound_dir, BT_SOUND_DIR);
E 3

D 3
  strcpy (sound_dir, BT_SOUND_DIR);
E 3
I 3
D 6
	/*
	 * Create empty first entry
	 */
	block->resize(1);
	block->operator[](0) = new BTSoundFile("mofo");
E 3

E 6
D 3
  // Create empty first entry
  block->resize( 1 );
  block->operator[](0) = new BTSoundFile("mofo");

  dir = opendir (strcat (sound_dir, dir_name));
  while ((dirent = readdir (dir)) != 0) {
    if (strstr (dirent->d_name, ".au") == 0)
      continue;
    block->resize (block->size() + 1);
    strcpy( name, dir_name );
    strcat( name, dirent->d_name );
    block->operator[](block->size() - 1) = new BTSoundFile( name );
  }
  closedir (dir);
E 3
I 3
	if ((dir = opendir(strcat(sound_dir, dir_name))) != NULL) {
		while ((dirent = readdir(dir)) != NULL) {
			if (strstr(dirent->d_name, ".au") == 0)
				continue;
			block->resize(block->size() + 1);
			strcpy(name, dir_name);
			strcat(name, dirent->d_name);
			block->operator[](block->size() - 1) =
			    new BTSoundFile(name);
  		}
		closedir (dir);
	}
E 3
  
D 3
  if(g_resources.r_rated == True && ((dir = opendir (strcat
						    (sound_dir,
						     ".r_rated"))) != 0)) {
E 3
I 3
	if(g_resources.r_rated == True &&
	    ((dir = opendir (strcat(sound_dir, ".r_rated"))) != NULL)) {
		char r_rated_name[BT_MAX_SND_FILENAME];

		strcpy(r_rated_name, dir_name);
		strcat(r_rated_name, ".r_rated/");
E 3
    
D 3
    char r_rated_name[BT_MAX_SND_FILENAME];
    strcpy (r_rated_name, dir_name);
    strcat (r_rated_name, ".r_rated/");
    
    while ((dirent = readdir (dir)) != 0) {
      if (strstr (dirent->d_name, ".au") == 0)
	continue;
      block->resize (block->size() + 1);
      strcpy( name, r_rated_name );
      strcat( name, dirent->d_name );
      block->operator[](block->size() - 1) = new BTSoundFile( name );
    }
    closedir (dir);
  }
E 3
I 3
		while ((dirent = readdir (dir)) != NULL) {
			if (strstr(dirent->d_name, ".au") == 0)
				continue;
			block->resize(block->size() + 1);
			strcpy(name, r_rated_name);
			strcat(name, dirent->d_name);
			block->operator[](block->size() - 1) =
			    new BTSoundFile(name);
		}
		closedir (dir);
	}
E 3

D 3
  // Mark all sounds as unplayed
  for ( int j = 0 ; j < block->size() ; j++ )
    (block->operator[](j))->used_ = 0;
E 3
I 3
	/*
	 * Mark all sounds as unplayed
	 */
	for (int j = 0 ; j < block->size(); j++)
		(block->operator[](j))->used_ = 0;
E 3
}

void BTSoundManager::play_random (Block<BTSoundFile *> *block) {
D 5
  if(block->size() <= 0)
    return;
E 5
I 5
	if(block->size() <= 0)
		return;
E 5

D 5
  int i;
E 5
I 5
	int i;
E 5

D 5
  // First make sure there is an unused entry
  if (block->operator[](0)->used_ == block->size()-1) {
    for (i = 0; i < block->size(); i++)
      (block->operator[](i))->used_ = 0;
  }
  for (;;) {
    i = (int) (drand48() * (block->size())) % block->size();
    if (i && (block->operator[](i)->used_ == 0))
      break;
  }
  block->operator[](i)->used_++;
  block->operator[](0)->used_++;
  *dev_ << (block->operator[](i))->name_;
E 5
I 5
	// First make sure there is an unused entry
	if (block->operator[](0)->used_ == block->size()-1) {
		for (i = 0; i < block->size(); i++)
			(block->operator[](i))->used_ = 0;
	}
	for (;;) {
		i = (int) (drand48() * (block->size())) % block->size();
		if (i && (block->operator[](i)->used_ == 0))
			break;
	}
	block->operator[](i)->used_++;
	block->operator[](0)->used_++;
	*dev_ << (block->operator[](i))->name_;
E 5
}


BTSoundManager::BTSoundManager (DevAudio *dev) 
D 5
: BTRingNode(), dev_ (dev), bad_move_ (0) {
E 5
I 5
	: BTRingNode(), dev_ (dev), bad_move_ (0) {
E 5

D 5
  paused_ = 0;
  happy_ = 0;
E 5
I 5
	paused_ = 0;
	happy_ = 0;
E 5

D 5
  int i;
  // First read in all of the idiot sounds
E 5
I 5
	int i;
	// First read in all of the idiot sounds
E 5

D 5
  read_dir ("idiot/", &bad_move_);
  read_dir ("welcome/", &welcome_);
  read_dir ("start/", &start_);
  read_dir ("near_death/", &near_death_);
  read_dir ("tetris/", &tetris_);
  read_dir ("survived/", &survived_);
  read_dir ("won/", &won_);
  read_dir ("lost/", &lost_);
  read_dir ("launched/", &launched_);
  read_dir ("launched/wimpy/", &launch_wimpy_);
E 5
I 5
	read_dir ("idiot/", &bad_move_);
	read_dir ("welcome/", &welcome_);
	read_dir ("start/", &start_);
	read_dir ("near_death/", &near_death_);
	read_dir ("tetris/", &tetris_);
	read_dir ("survived/", &survived_);
	read_dir ("won/", &won_);
	read_dir ("lost/", &lost_);
	read_dir ("launched/", &launched_);
	read_dir ("launched/wimpy/", &launch_wimpy_);
E 5

}  

void BTSoundManager::receive (BTRingPacket *packet) {

D 5
  if (dev_)
    switch (packet->token) {
    case BT_SCORE: {
      break;
    }
    case BT_LINE: {
      if (((BTLine *) packet->data)->inc() >= 4 
          && ((BTLine *) packet->data)->inc() < 8) {
        play_random (&tetris_);
      }
      if (*((BTLine *) packet->data) == 8)
        *dev_ << "misc/sally.au";
      if ((*((BTLine *) packet->data) == 1)) {
	     if (happy_) 
	       *dev_ << "misc/im_so_happy.au";
	       happy_ = 0;
      }
      break;
    }
    case BT_FUNDS: {
      if (*((short *) packet->data) >= BT_HAPPY_VAL)
	      happy_ = 1;
      else
	      happy_ = 0;
      break;
    }
    case BT_IDIOT: {
      switch (*((short *) packet->data)) {
      case BT_BAD_MOVE: {
        play_random (&bad_move_);
	      break;
      }
      case BT_NEAR_DEATH: {
        if (rand() % 4 == 0) 
          play_random (&near_death_);
	      break;
      }
      case BT_MISSED_SMILEY: {
	*dev_ << "misc/doh.au";
        break;
      }
      }
      break;
    }
    case BT_DEAD: {
      play_random (&won_);
      break;
    }
    case BT_GAME_OVER: {
      if (!packet->data) {
        play_random (&lost_);
        break;
      }
    }
    case BT_PAUSE: {
      if (!paused_) {
	*dev_ << "misc/freeze_program.au";
	paused_ = 1;
      } else paused_ = 0;
      break;
    }  
    case BT_WPN_ON: {
      BTWeapon *wpn = (BTWeapon *) packet->data;
      switch (wpn->token()) {
      case BT_REAGAN: 
	      *dev_ << "weapons/flush.au";
	      break;
      case BT_FEARED_WEIRD: 
	      *dev_ << "weapons/sub_dive_horn.au";
	      break;
      case BT_BUG:
      case BT_PIECE_IT: 
	      *dev_ << "weapons/Bottle.au";
	      break;
      case BT_FALL_OUT:
	      *dev_ << "weapons/swish.au";
	      break;
      case BT_HATTER:
	      *dev_ << "weapons/haveyou_gone_mad.au";
	      break;
      case BT_KEATING:
	      *dev_ << "weapons/Got_Your_Nose.au";
	      break;
      case BT_SPEEDY:
	      *dev_ << "weapons/MeepMeep.au";
	      break;
      case BT_UPBYSIDE:
	      *dev_ << "weapons/World_Gone_TopsyTurvy.au";
	      break;
      case BT_GIMP:
	      if(g_resources.r_rated)
		*dev_ << "misc/.r_rated/gimp.au";
	      else
		*dev_ << "weapons/cmon.au";
	      break;
      case BT_SUSAN:
        *dev_ << "weapons/give_to_me.au";
        break;
      case BT_CONDOR:
      case BT_AMES:
      case BT_ACE:
//	Nay, bmc
//        *dev_ << "weapons/Sonar.au";
        break;
      case BT_BLIND:
	      *dev_ << "weapons/Explosion-2.au";
	      break;
      case BT_RISE_UP:
        *dev_ << "weapons/jeffersons.au";
        break;
      case BT_FORCE:
        *dev_ << "weapons/force.au";
        break;
      case BT_FOUR_BY_FOUR:
	      *dev_ << "weapons/abandon-ship.au";
	      break;
      case BT_SLICK:
        *dev_ << "weapons/clinton.au";
        break;
      case BT_TWILIGHT:
        *dev_ << "weapons/TwilightZone.au";
        break;
    }
    break;
    }
    case BT_START: 
      break;
E 5
I 5
	if (dev_)
		switch (packet->token) {
		case BT_SCORE: {
			break;
		}
		case BT_LINE: {
			if (((BTLine *) packet->data)->inc() >= 4 
			    && ((BTLine *) packet->data)->inc() < 8) {
				play_random (&tetris_);
			}
			if (*((BTLine *) packet->data) == 8)
				*dev_ << "misc/sally.au";
			if ((*((BTLine *) packet->data) == 1)) {
				if (happy_) 
					*dev_ << "misc/im_so_happy.au";
				happy_ = 0;
			}
			break;
		}
		case BT_FUNDS: {
			if (*((short *) packet->data) >= BT_HAPPY_VAL)
				happy_ = 1;
			else
				happy_ = 0;
			break;
		}
		case BT_IDIOT: {
			switch (*((short *) packet->data)) {
			case BT_BAD_MOVE: {
				play_random (&bad_move_);
				break;
			}
			case BT_NEAR_DEATH: {
				if (rand() % 4 == 0) 
					play_random (&near_death_);
				break;
			}
			case BT_MISSED_SMILEY: {
				*dev_ << "misc/doh.au";
				break;
			}
			}
			break;
		}
		case BT_DEAD: {
			play_random (&won_);
			break;
		}
		case BT_GAME_OVER:
			if (!packet->data)
				play_random (&lost_);
			break;
E 5

D 5
    case BT_WPN_OFF: {
      BTWeapon *wpn = (BTWeapon *) packet->data;
      switch (wpn->token()) {
      case BT_FOUR_BY_FOUR:
      case BT_FEARED_WEIRD:
      case BT_SLICK:
      case BT_HATTER:
        play_random (&survived_);
        break;
      }
      break;
    }
    case BT_WPN_LAUNCH: {
      BTWeapon *wpn = (BTWeapon *) packet->data;
      int sound_no = 1;
      switch(wpn->token()) {
      case BT_GIMP:
	*dev_ << "weapons/gimp.au";
	sound_no--;
	break;
      default:
	break;
      }
      if ( sound_no == 0 )
	break;
      if (wpn->price_ <= 100) {
        play_random (&launch_wimpy_);
        break;
      }
      if (wpn->price_ > 400) {
        play_random (&launched_);
        break;
      }
    }
  }
  pass (packet);
E 5
I 5
		case BT_AIRSLIDE:
			*dev_ << "tetris/hoooah.au";
			break;

		case BT_PAUSE: {
			if (!paused_) {
				*dev_ << "misc/freeze_program.au";
				paused_ = 1;
			} else paused_ = 0;
			break;
		}  
		case BT_WPN_ON: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			switch (wpn->token()) {
			case BT_REAGAN: 
				*dev_ << "weapons/flush.au";
				break;
			case BT_FEARED_WEIRD: 
				*dev_ << "weapons/sub_dive_horn.au";
				break;
			case BT_BUG:
			case BT_PIECE_IT: 
				*dev_ << "weapons/Bottle.au";
				break;
			case BT_FALL_OUT:
				*dev_ << "weapons/swish.au";
				break;
			case BT_HATTER:
				*dev_ << "weapons/haveyou_gone_mad.au";
				break;
			case BT_KEATING:
				*dev_ << "weapons/Got_Your_Nose.au";
				break;
			case BT_SPEEDY:
				*dev_ << "weapons/MeepMeep.au";
				break;
			case BT_UPBYSIDE:
				*dev_ << "weapons/World_Gone_TopsyTurvy.au";
				break;
			case BT_GIMP:
				if(g_resources.r_rated)
					*dev_ << "misc/.r_rated/gimp.au";
				else
					*dev_ << "weapons/cmon.au";
				break;
			case BT_SUSAN:
				*dev_ << "weapons/give_to_me.au";
				break;
			case BT_CONDOR:
			case BT_AMES:
			case BT_ACE:
				//	Nay, bmc
				//        *dev_ << "weapons/Sonar.au";
				break;
			case BT_BLIND:
				*dev_ << "weapons/Explosion-2.au";
				break;
			case BT_RISE_UP:
				*dev_ << "weapons/jeffersons.au";
				break;
			case BT_FORCE:
				*dev_ << "weapons/force.au";
				break;
			case BT_FOUR_BY_FOUR:
				*dev_ << "weapons/abandon-ship.au";
				break;
			case BT_SLICK:
				*dev_ << "weapons/clinton.au";
				break;
			case BT_TWILIGHT:
				*dev_ << "weapons/TwilightZone.au";
				break;
			}
			break;
		}
		case BT_START: 
			break;

		case BT_WPN_OFF: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			switch (wpn->token()) {
			case BT_FOUR_BY_FOUR:
			case BT_FEARED_WEIRD:
			case BT_SLICK:
			case BT_HATTER:
				play_random (&survived_);
				break;
			}
			break;
		}
		case BT_WPN_LAUNCH: {
			BTWeapon *wpn = (BTWeapon *) packet->data;
			int sound_no = 1;
			switch(wpn->token()) {
			case BT_GIMP:
				*dev_ << "weapons/gimp.au";
				sound_no--;
				break;
			default:
				break;
			}
			if ( sound_no == 0 )
				break;
			if (wpn->price_ <= 100) {
				play_random (&launch_wimpy_);
				break;
			}
			if (wpn->price_ > 400) {
				play_random (&launched_);
				break;
			}
		}
		}
	pass (packet);
E 5
}

void BTSoundManager::welcome() {
D 5
  register int i;
  time_t curtime;
E 5
I 5
	register int i;
	time_t curtime;
E 5

D 5
  time(&curtime);
  srand48(curtime + getpid());
E 5
I 5
	time(&curtime);
	srand48(curtime + getpid());
E 5

D 5
  for(i = (curtime % 100); i; i--)
    lrand48();
E 5
I 5
	for(i = (curtime % 100); i; i--)
		lrand48();
E 5

D 5
  if(dev_ && welcome_.size() > 0)
    *dev_ << welcome_[i = lrand48() % welcome_.size()]->name_;
E 5
I 5
	if(dev_ && welcome_.size() > 0)
		*dev_ << welcome_[i = lrand48() % welcome_.size()]->name_;
E 5
}

void BTSoundManager::playJeopardy() {
D 5
  if (dev_)
    *dev_ << "misc/jeopardy.au";
E 5
I 5
	if (dev_)
		*dev_ << "misc/jeopardy.au";
E 5
}

void BTSoundManager::start() {
D 5
  if (dev_) 
    play_random (&start_);
E 5
I 5
	if (dev_) 
		play_random (&start_);
E 5
}

void BTSoundManager::cleanBlock( Block<BTSoundFile *> *block ) {
D 5
  for ( int j = 0 ; j < block->size() ; j++ )
    delete block->operator[](j);
E 5
I 5
	for ( int j = 0 ; j < block->size() ; j++ )
		delete block->operator[](j);
E 5
}

BTSoundManager::~BTSoundManager() {
D 5
  cleanBlock(&bad_move_);
  cleanBlock(&welcome_);
  cleanBlock(&start_);
  cleanBlock(&near_death_);
  cleanBlock(&tetris_);
  cleanBlock(&survived_);
  cleanBlock(&won_);
  cleanBlock(&lost_);
  cleanBlock(&launched_);
  cleanBlock(&launch_wimpy_);
E 5
I 5
	cleanBlock(&bad_move_);
	cleanBlock(&welcome_);
	cleanBlock(&start_);
	cleanBlock(&near_death_);
	cleanBlock(&tetris_);
	cleanBlock(&survived_);
	cleanBlock(&won_);
	cleanBlock(&lost_);
	cleanBlock(&launched_);
	cleanBlock(&launch_wimpy_);
E 5
}
E 1
