/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTConstants.H                                       */
/*    DATE: Sun Feb  6 16:15:36 1994                            */
/****************************************************************/

#ifndef BTCONSTANTS_H
#define BTCONSTANTS_H

// Version information
#define BT_VERSION	"BattleTris v1.0"
#define BT_MAJOR_VER	1
#define BT_MINOR_VER	0

// Constants for obtaining information about the current user
#define BT_USERNAMELEN	263
#define BT_GROUPNAMELEN	8
#define BT_GECOSNAMELEN	255
#define BT_HOSTABBRLEN	255
#define BT_HOSTNAMELEN	255
#define BT_PLANFILELEN	1024
#define BT_NICKNAMELEN	32
#define BT_COOKIELEN	8

// Constants for computing player rank value
#define BT_ELO_START	1200L

// Constants for piece colors
#define BT_MAX_DIF_COLORS	9

#define BT_INVISIBLE -1
#define BT_BLACK      0
#define BT_IVORY      1
#define BT_YELLOW     2
#define BT_RED        3
#define BT_BLUE       4
#define BT_ORANGE     5
#define BT_GREEN      6
#define BT_CYAN       7
#define BT_PURPLE     8

#define BT_NEUTRAL    9

#define BT_GRAY           BT_IVORY + BT_MAX_DIF_COLORS
#define BT_DYELLOW        BT_YELLOW + BT_MAX_DIF_COLORS
#define BT_DRED           BT_RED + BT_MAX_DIF_COLORS
#define BT_DBLUE          BT_BLUE + BT_MAX_DIF_COLORS
#define BT_DORANGE        BT_ORANGE + BT_MAX_DIF_COLORS
#define BT_DGREEN         BT_GREEN + BT_MAX_DIF_COLORS
#define BT_DCYAN          BT_CYAN + BT_MAX_DIF_COLORS
#define BT_DPURPLE        BT_PURPLE + BT_MAX_DIF_COLORS
#define BT_MAX_COLORS     BT_NEUTRAL + BT_MAX_DIF_COLORS

#define BT_STRUCT     20

#define BT_HAPPY      21
#define BT_UNHAPPY    22
#define BT_GIMP_ID       23

#define BT_DIE_1      24
#define BT_DIE_2      25
#define BT_DIE_3      26
#define BT_DIE_4      27
#define BT_DIE_5      28
#define BT_DIE_6      29

#define BT_MAX_BOXES  30

#define BT_BOX_WTH 23
#define BT_BOX_HGT 23
#define BT_BOX_BRDR 3

#define BT_DIE_RAD 5
#define BT_DIE_X1 1
#define BT_DIE_X2 7
#define BT_DIE_X3 13
#define BT_DIE_Y1 1
#define BT_DIE_Y2 7
#define BT_DIE_Y3 13

#define BT_HAPPY_VAL 150

#define BT_BOX_ID_OFFS        0
#define BT_DIE_ID_OFFS        100     
#define BT_HAPPY_ID_OFFS      200
#define BT_GIMP_ID_OFFS       300

#define BT_BOARD_WTH 10
#define BT_BOARD_HGT 28

#define BT_FAST_DROP_TIME 10
#define BT_DROP_TIME 512
#define BT_SLIDE_TIME 150

#define BT_BASE_PROB .21

#define BT_DEFAULT_X 5
#define BT_DEFAULT_Y 0

#define BT_PIECE_WIDTH 8 
#define BT_PIECE_HEIGHT 8 

#define BT_EL_PIECE      1 
#define BT_REL_PIECE     2
#define BT_SL_RT_PIECE   3
#define BT_SL_LF_PIECE   4
#define BT_LONG_PIECE    5
#define BT_PLUG_PIECE    6
#define BT_BOX_PIECE     7

#define BT_DIE_PIECE     8
#define BT_HAP_PIECE     9

#define BT_WEIRD_OFFS    9
#define BT_DOG_PIECE    10
#define BT_RDOG_PIECE   11
#define BT_CAP_PIECE    12
#define BT_WALL_PIECE   13
#define BT_TOWER_PIECE  14
#define BT_STAR_PIECE   15
#define BT_WLONG_PIECE  16

#define BT_4x4_PIECE       17
#define BT_LONG_DONG_PIECE 18
#define BT_MAX_PIECES 18

// These are idiot specifics
#define BT_BAD_MOVE      0
#define BT_NEAR_DEATH    1
#define BT_MISSED_SMILEY 2

// Various weapon constants
#define BT_FALL_OUT_LEDGE 2
#define BT_ARSENAL_SIZE 10

#endif
