MEMORY
{
  /* The 0x8000 first bytes of FLASH are reserved for the bootloader */
  FLASH : ORIGIN = 0x08000000, LENGTH = 0x8000
  /* The 0x10 first bytes of RAM are reserved for the bootloader magic word */
  RAM : ORIGIN = 0x20000010, LENGTH = 128K - 0x10
}
